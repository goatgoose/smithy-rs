/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

use aws_http::user_agent::AwsUserAgent;
use aws_runtime::invocation_id::{InvocationId, PredefinedInvocationIdGenerator};
use aws_sdk_s3::config::interceptors::BeforeSerializationInterceptorContextMut;
use aws_sdk_s3::config::interceptors::FinalizerInterceptorContextRef;
use aws_sdk_s3::config::retry::RetryConfig;
use aws_sdk_s3::config::timeout::TimeoutConfig;
use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::config::{Interceptor, SharedAsyncSleep};
use aws_sdk_s3::Client;
use aws_smithy_async::test_util::InstantSleep;
use aws_smithy_async::test_util::ManualTimeSource;
use aws_smithy_async::time::SharedTimeSource;
use aws_smithy_protocol_test::MediaType;
use aws_smithy_runtime::client::http::test_util::dvr::ReplayingClient;
use aws_smithy_runtime::test_util::capture_test_logs::capture_test_logs;
use aws_smithy_runtime_api::box_error::BoxError;
use aws_smithy_runtime_api::client::runtime_components::RuntimeComponents;
use aws_smithy_types::config_bag::{ConfigBag, Layer};
use std::time::{Duration, UNIX_EPOCH};

#[derive(Debug)]
struct TimeInterceptor {
    time_source: ManualTimeSource,
}
impl Interceptor for TimeInterceptor {
    fn name(&self) -> &'static str {
        "TimeInterceptor"
    }

    fn modify_before_serialization(
        &self,
        _context: &mut BeforeSerializationInterceptorContextMut<'_>,
        _runtime_components: &RuntimeComponents,
        cfg: &mut ConfigBag,
    ) -> Result<(), BoxError> {
        let mut layer = Layer::new("test");
        layer.store_put(AwsUserAgent::for_tests());
        cfg.push_layer(layer);
        Ok(())
    }

    fn read_after_attempt(
        &self,
        _context: &FinalizerInterceptorContextRef<'_>,
        _runtime_components: &RuntimeComponents,
        _cfg: &mut ConfigBag,
    ) -> Result<(), BoxError> {
        self.time_source.advance(Duration::from_secs(1));
        tracing::info!(
            "################ ADVANCED TIME BY 1 SECOND, {:?}",
            &self.time_source
        );
        Ok(())
    }
}

// One SDK operation invocation.
// Client retries 3 times, successful response on 3rd attempt.
// Fast network, latency + server time is less than one second.
// No clock skew
// Client waits 1 second between retry attempts.
#[tokio::test]
async fn three_retries_and_then_success() {
    let _logs = capture_test_logs();

    let time_source = ManualTimeSource::new(UNIX_EPOCH + Duration::from_secs(1559347200));

    let path = "tests/data/request-information-headers/three-retries_and-then-success.json";
    let http_client = ReplayingClient::from_file(path).unwrap();
    let config = aws_sdk_s3::Config::builder()
        .credentials_provider(Credentials::for_tests_with_session_token())
        .region(Region::new("us-east-1"))
        .http_client(http_client.clone())
        .time_source(SharedTimeSource::new(time_source.clone()))
        .sleep_impl(SharedAsyncSleep::new(InstantSleep::new(Default::default())))
        .retry_config(RetryConfig::standard())
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(10))
                .build(),
        )
        .invocation_id_generator(PredefinedInvocationIdGenerator::new(vec![
            InvocationId::new_from_str("00000000-0000-4000-8000-000000000000"),
        ]))
        .interceptor(TimeInterceptor { time_source })
        .build();
    let client = Client::from_conf(config);

    let resp = dbg!(
        client
            .list_objects_v2()
            .bucket("test-bucket")
            .prefix("prefix~")
            .send()
            .await
    );

    let resp = resp.expect("valid e2e test");
    assert_eq!(resp.name(), Some("test-bucket"));
    http_client
        .full_validate(MediaType::Xml)
        .await
        .expect("failed")
}

// Client makes 3 separate SDK operation invocations
// All succeed on first attempt.
// Fast network, latency + server time is less than one second.
#[tokio::test]
async fn three_successful_attempts() {
    let _logs = capture_test_logs();

    let time_source = ManualTimeSource::new(UNIX_EPOCH + Duration::from_secs(1559347200));

    let path = "tests/data/request-information-headers/three-successful-attempts.json";
    let http_client = ReplayingClient::from_file(path).unwrap();
    let config = aws_sdk_s3::Config::builder()
        .credentials_provider(Credentials::for_tests_with_session_token())
        .region(Region::new("us-east-1"))
        .http_client(http_client.clone())
        .time_source(SharedTimeSource::new(time_source.clone()))
        .sleep_impl(SharedAsyncSleep::new(InstantSleep::new(Default::default())))
        .retry_config(RetryConfig::standard())
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(10))
                .build(),
        )
        .invocation_id_generator(PredefinedInvocationIdGenerator::new(vec![
            InvocationId::new_from_str("3dfe4f26-c090-4887-8c14-7bac778bca07"),
            InvocationId::new_from_str("70370531-7b83-4b90-8b93-46975687ecf6"),
            InvocationId::new_from_str("910bf450-6c90-43de-a508-3fa126a06b71"),
        ]))
        .interceptor(TimeInterceptor { time_source })
        .build();
    let client = Client::from_conf(config);

    let resp = dbg!(
        client
            .list_objects_v2()
            .bucket("test-bucket")
            .prefix("prefix~")
            .send()
            .await
    );

    let resp = resp.expect("valid e2e test");
    assert_eq!(resp.name(), Some("test-bucket"));

    let resp = dbg!(
        client
            .list_objects_v2()
            .bucket("test-bucket")
            .prefix("prefix~")
            .send()
            .await
    );

    let resp = resp.expect("valid e2e test");
    assert_eq!(resp.name(), Some("test-bucket"));

    let resp = dbg!(
        client
            .list_objects_v2()
            .bucket("test-bucket")
            .prefix("prefix~")
            .send()
            .await
    );

    let resp = resp.expect("valid e2e test");
    assert_eq!(resp.name(), Some("test-bucket"));

    http_client
        .full_validate(MediaType::Xml)
        .await
        .expect("failed")
}

// One SDK operation invocation.
// Client retries 3 times, successful response on 3rd attempt.
// Slow network, one way latency is 2 seconds.
// Server takes 1 second to generate response.
// Client clock is 10 minutes behind server clock.
// One second delay between retries.
#[tokio::test]
async fn slow_network_and_late_client_clock() {
    let _logs = capture_test_logs();

    let time_source = ManualTimeSource::new(UNIX_EPOCH + Duration::from_secs(1559347200));

    let path = "tests/data/request-information-headers/slow-network-and-late-client-clock.json";
    let http_client = ReplayingClient::from_file(path).unwrap();
    let config = aws_sdk_s3::Config::builder()
        .credentials_provider(Credentials::for_tests_with_session_token())
        .region(Region::new("us-east-1"))
        .http_client(http_client.clone())
        .time_source(SharedTimeSource::new(time_source.clone()))
        .sleep_impl(SharedAsyncSleep::new(InstantSleep::new(Default::default())))
        .retry_config(RetryConfig::standard())
        .timeout_config(
            TimeoutConfig::builder()
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(10))
                .build(),
        )
        .invocation_id_generator(PredefinedInvocationIdGenerator::new(vec![
            InvocationId::new_from_str("3dfe4f26-c090-4887-8c14-7bac778bca07"),
        ]))
        .interceptor(TimeInterceptor { time_source })
        .build();
    let client = Client::from_conf(config);

    let resp = dbg!(
        client
            .list_objects_v2()
            .bucket("test-bucket")
            .prefix("prefix~")
            .send()
            .await
    );

    let resp = resp.expect("valid e2e test");
    assert_eq!(resp.name(), Some("test-bucket"));
    http_client
        .full_validate(MediaType::Xml)
        .await
        .expect("failed")
}
