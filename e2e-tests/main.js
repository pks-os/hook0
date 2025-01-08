import { getEnvironmentVariables } from './config.js';
import create_application from './applications/create_application.js';
import create_event_type from './event_types/create_event_type.js';
import create_subscription from './subscriptions/create_subscription.js';
import send_event from './events/send_event.js';
import list_request_attempt from './events/list_request_attempt.js';
import delete_application from './applications/delete_application.js';

export const config = getEnvironmentVariables();

export const options = {
  vus: config.vus,
  duration: config.duration,
  //maxDuration: config.maxDuration,
};

function isNotNull(value) {
  return value && value !== null;
}

function scenario_1() {
  const h = config.apiOrigin;
  const s = config.serviceToken;
  const o = config.organizationId;

  let application_id = create_application(h, o, s);
  if (!isNotNull(application_id)) {
    console.error('Failed to create application');
    return;
  }

  let event_type_1 = create_event_type(h, s, application_id);
  if (!isNotNull(event_type_1)) {
    console.error('Failed to create event type 1');
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }

  let event_type_2 = create_event_type(h, s, application_id);
  if (!isNotNull(event_type_2)) {
    console.error('Failed to create event type 2');
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }

  let subscription_1 = create_subscription(
    h,
    s,
    application_id,
    [event_type_1, event_type_2],
    config.targetUrl,
    'all',
    'yes'
  );
  if (!isNotNull(subscription_1)) {
    console.error('Failed to create subscription');
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }
  let subscription_1_id = subscription_1.subscription_id;
  let label_1_key = subscription_1.label_key;
  let label_1_value = subscription_1.label_value;

  let subscription_2 = create_subscription(
    h,
    s,
    application_id,
    [event_type_1],
    config.targetUrl,
    'all',
    'yes'
  );
  if (!isNotNull(subscription_2)) {
    console.error('Failed to create subscription');
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }
  let subscription_2_id = subscription_2.subscription_id;
  let label_2_key = subscription_2.label_key;
  let label_2_value = subscription_2.label_value;

  let event_1 = send_event(s, h, application_id, event_type_1, { [label_1_key]: label_1_value });
  if (!isNotNull(event_1)) {
    console.error('Failed to create event 1');
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }

  let event_2 = send_event(s, h, application_id, event_type_2, { [label_2_key]: label_2_value });
  if (!isNotNull(event_2)) {
    console.error('Failed to create event 2');
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }

  // Event 3 shoudn't be delivered
  let event_3 = send_event(s, h, application_id, event_type_1, { test: 'test' });
  if (!isNotNull(event_3)) {
    console.error('Failed to create event 3');
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }

  let request_attempts_1 = list_request_attempt(h, s, application_id, event_1);
  if (!isNotNull(request_attempts_1)) {
    console.error('Failed to list request attempts 1');
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }
  if (request_attempts_1.length !== 2) {
    console.error(
      'Expected to find 2 request attempts for event 1 | Found: ' + request_attempts_1.length
    );
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }

  let request_attempts_2 = list_request_attempt(h, s, application_id, event_2);
  if (!isNotNull(request_attempts_2)) {
    console.error('Failed to list request attempts 2');
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }
  if (request_attempts_2.length !== 1) {
    console.error(
      'Expected to find 1 request attempts for event 2 | Found: ' + request_attempts_2.length
    );
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }

  // Event 3 soulnd't be received by any subscription so the length should be 0
  let request_attempts_3 = list_request_attempt(h, s, application_id, event_3);
  if (!isNotNull(request_attempts_3)) {
    console.error('Failed to list request attempts 3');
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }
  if (request_attempts_3.length !== 0) {
    console.error(
      'Expected to find 0 request attempts for event 3 | Found: ' + request_attempts_3.length
    );
    if (config.deleteOnFail) delete_application(h, application_id, s);
    return;
  }
}

export default function () {
  scenario_1();
}
