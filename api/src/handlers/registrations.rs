use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHasher};
use lettre::message::Mailbox;
use lettre::Address;
use log::{error, warn};
use paperclip::actix::web::{Data, Json};
use paperclip::actix::{api_v2_operation, Apiv2Schema, CreatedJson};
use serde::{Deserialize, Serialize};
use sqlx::query;
use std::str::FromStr;
use uuid::Uuid;
use validator::Validate;

use crate::iam::{create_email_verification_token, Role};
use crate::mailer::Mail;
use crate::problems::Hook0Problem;

#[derive(Debug, Serialize, Apiv2Schema)]
pub struct Registration {
    organization_id: Uuid,
    user_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, Apiv2Schema, Validate)]
pub struct RegistrationPost {
    #[validate(non_control_character, length(min = 1, max = 50))]
    first_name: String,
    #[validate(non_control_character, length(min = 1, max = 50))]
    last_name: String,
    #[validate(non_control_character, email, length(max = 100))]
    email: String,
    #[validate(
        non_control_character,
        length(
            min = 10,
            max = 100,
            message = "Password must be at least 10 characters long and at most 100 characters long"
        )
    )]
    password: String,
}

#[api_v2_operation(
    summary = "Create a new user account and its own personal organization",
    description = "",
    operation_id = "register",
    consumes = "application/json",
    produces = "application/json",
    tags("Organizations Management")
)]
pub async fn register(
    state: Data<crate::State>,
    body: Json<RegistrationPost>,
) -> Result<CreatedJson<Registration>, Hook0Problem> {
    if state.registration_disabled {
        return Err(Hook0Problem::RegistrationDisabled);
    }

    if let Err(e) = body.validate() {
        return Err(Hook0Problem::Validation(e));
    }

    let recipient_address = Address::from_str(&body.email).map_err(|e| {
        // Should not happen because we checked (using a validator) that body.email is a well structured email address
        error!("Error trying to parse email address: {e}");
        Hook0Problem::InternalServerError
    })?;

    if body.password.len() >= usize::from(state.password_minimum_length) {
        let mut tx = state.db.begin().await?;

        let user_id = Uuid::new_v4();
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(body.password.as_bytes(), &salt)
            .map_err(|e| {
                error!("Error trying to hash user password: {e}");
                Hook0Problem::InternalServerError
            })?
            .serialize();
        let user_insert = query!(
            "
                INSERT INTO iam.user (user__id, email, password, first_name, last_name)
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (email) DO NOTHING
            ",
            &user_id,
            &body.email,
            password_hash.as_str(),
            &body.first_name,
            &body.last_name,
        )
        .execute(&mut *tx)
        .await?;

        if user_insert.rows_affected() > 0 {
            let organization_id = Uuid::new_v4();
            let organization_name = format!(
                "{} {}'s personal organization",
                &body.first_name, &body.last_name
            );
            query!(
                "
                    INSERT INTO iam.organization (organization__id, name, created_by)
                    VALUES ($1, $2, $3)
                ",
                &organization_id,
                &organization_name,
                &user_id,
            )
            .execute(&mut *tx)
            .await?;

            query!(
                "
                    INSERT INTO iam.user__organization (user__id, organization__id, role)
                    VALUES ($1, $2, $3)
                ",
                &user_id,
                &organization_id,
                Role::Editor.as_ref(),
            )
            .execute(&mut *tx)
            .await?;

            let verification_token =
                create_email_verification_token(&state.biscuit_private_key, user_id).map_err(
                    |e| {
                        error!("Error trying to create email verification token: {e}");
                        Hook0Problem::InternalServerError
                    },
                )?;
            let recipient = Mailbox::new(
                Some(format!("{} {}", body.first_name, body.last_name)),
                recipient_address,
            );
            state
                .mailer
                .send_mail(
                    Mail::VerifyUserEmail {
                        url: format!(
                            "{}/verify-email?token={}",
                            state.app_url, &verification_token.serialized_biscuit
                        ),
                    },
                    recipient,
                )
                .await
                .map_err(|e| {
                    warn!("Could not send verification email: {e}");
                    e
                })?;

            tx.commit().await?;

            Ok(CreatedJson(Registration {
                organization_id,
                user_id,
            }))
        } else {
            Err(Hook0Problem::UserAlreadyExist)
        }
    } else {
        Err(Hook0Problem::PasswordTooShort(
            state.password_minimum_length,
        ))
    }
}
