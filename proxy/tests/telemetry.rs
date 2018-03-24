#[macro_use]
extern crate log;

mod support;
use self::support::*;

macro_rules! assert_contains {
    ($scrape:expr, $contains:expr) => {
        assert!($scrape.contains($contains), "metrics scrape:\n{:8}\ndid not contain:\n{:8}", $scrape, $contains)
    }
}

#[test]
fn metrics_endpoint_inbound_request_count() {
    let _ = env_logger::try_init();

    info!("running test server");
    let srv = server::new().route("/hey", "hello").run();

    let ctrl = controller::new();
    let proxy = proxy::new()
        .controller(ctrl.run())
        .inbound(srv)
        .run();
    let client = client::new(proxy.inbound, "tele.test.svc.cluster.local");
    let metrics = client::http1(proxy.metrics, "localhost");

    // prior to seeing any requests, request count should be empty.
    assert!(!metrics.get("/metrics")
        .contains("request_total{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\"}"));

    info!("client.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    // after seeing a request, the request count should be 1.
    assert_contains!(scrape, "request_total{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\"} 1");

}

#[test]
fn metrics_endpoint_outbound_request_count() {
    let _ = env_logger::try_init();

    info!("running test server");
    let srv = server::new().route("/hey", "hello").run();

    let ctrl = controller::new()
        .destination("tele.test.svc.cluster.local", srv.addr)
        .run();
    let proxy = proxy::new()
        .controller(ctrl)
        .outbound(srv)
        .run();
    let client = client::new(proxy.outbound, "tele.test.svc.cluster.local");
    let metrics = client::http1(proxy.metrics, "localhost");

    // prior to seeing any requests, request count should be empty.
    assert!(!metrics.get("/metrics")
        .contains("request_total{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\"}"));

    info!("client.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    // after seeing a request, the request count should be 1.
    assert_contains!(scrape, "request_total{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\"} 1");

}

// Ignore this test on CI, because our method of adding latency to requests
// (calling `thread::sleep`) is likely to be flakey on Travis.
// Eventually, we can add some kind of mock timer system for simulating latency
// more reliably, and re-enable this test.
#[test]
#[cfg_attr(not(feature = "flaky_tests"), ignore)]
fn metrics_endpoint_inbound_response_latency() {
    let _ = env_logger::try_init();

    info!("running test server");
    let srv = server::new()
        .route_with_latency("/hey", "hello", Duration::from_millis(500))
        .route_with_latency("/hi", "good morning", Duration::from_millis(40))
        .run();

    let ctrl = controller::new();
    let proxy = proxy::new()
        .controller(ctrl.run())
        .inbound(srv)
        .run();
    let client = client::new(proxy.inbound, "tele.test.svc.cluster.local");
    let metrics = client::http1(proxy.metrics, "localhost");

    info!("client.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    // assert the >=1000ms bucket is incremented by our request with 500ms
    // extra latency.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\",status_code=\"200\",le=\"1000\"} 1");
    // the histogram's count should be 1.
    assert_contains!(scrape,
        "response_latency_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\",status_code=\"200\"} 1");
    // TODO: we're not going to make any assertions about the
    // response_latency_ms_sum stat, since its granularity depends on the actual
    // observed latencies, which may vary a bit. we could make more reliable
    // assertions about that stat if we were using a mock timer, though, as the
    // observed latency values would be predictable.

    info!("client.get(/hi)");
    assert_eq!(client.get("/hi"), "good morning");

    let scrape = metrics.get("/metrics");

    // request with 40ms extra latency should fall into the 50ms bucket.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\",status_code=\"200\",le=\"50\"} 1");
    // 1000ms bucket should be incremented as well, since it counts *all*
    // bservations less than or equal to 1000ms, even if they also increment
    // other buckets.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\",status_code=\"200\",le=\"1000\"} 2");
    // the histogram's total count should be 2.
    assert_contains!(scrape,
        "response_latency_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\",status_code=\"200\"} 2");

    info!("client.get(/hi)");
    assert_eq!(client.get("/hi"), "good morning");

    let scrape = metrics.get("/metrics");
    // request with 40ms extra latency should fall into the 50ms bucket.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\",status_code=\"200\",le=\"50\"} 2");
    // 1000ms bucket should be incremented as well.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\",status_code=\"200\",le=\"1000\"} 3");
    // the histogram's total count should be 3.
    assert_contains!(scrape,
        "response_latency_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\",status_code=\"200\"} 3");

    info!("client.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    // 50ms bucket should be un-changed by the request with 500ms latency.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\",status_code=\"200\",le=\"50\"} 2");
    // 1000ms bucket should be incremented.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\",status_code=\"200\",le=\"1000\"} 4");
    // the histogram's total count should be 4.
    assert_contains!(scrape,
        "response_latency_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\",status_code=\"200\"} 4");
}

// Ignore this test on CI, because our method of adding latency to requests
// (calling `thread::sleep`) is likely to be flakey on Travis.
// Eventually, we can add some kind of mock timer system for simulating latency
// more reliably, and re-enable this test.
#[test]
#[cfg_attr(not(feature = "flaky_tests"), ignore)]
fn metrics_endpoint_outbound_response_latency() {
    let _ = env_logger::try_init();

    info!("running test server");
    let srv = server::new()
        .route_with_latency("/hey", "hello", Duration::from_millis(500))
        .route_with_latency("/hi", "good morning", Duration::from_millis(40))
        .run();

    let ctrl = controller::new()
        .destination("tele.test.svc.cluster.local", srv.addr)
        .run();
    let proxy = proxy::new()
        .controller(ctrl)
        .outbound(srv)
        .run();
    let client = client::new(proxy.outbound, "tele.test.svc.cluster.local");
    let metrics = client::http1(proxy.metrics, "localhost");

    info!("client.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    // assert the >=1000ms bucket is incremented by our request with 500ms
    // extra latency.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\",status_code=\"200\",le=\"1000\"} 1");
    // the histogram's count should be 1.
    assert_contains!(scrape,
        "response_latency_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\",status_code=\"200\"} 1");
    // TODO: we're not going to make any assertions about the
    // response_latency_ms_sum stat, since its granularity depends on the actual
    // observed latencies, which may vary a bit. we could make more reliable
    // assertions about that stat if we were using a mock timer, though, as the
    // observed latency values would be predictable.

    info!("client.get(/hi)");
    assert_eq!(client.get("/hi"), "good morning");

    let scrape = metrics.get("/metrics");

    // request with 40ms extra latency should fall into the 50ms bucket.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\",status_code=\"200\",le=\"50\"} 1");
    // 1000ms bucket should be incremented as well, since it counts *all*
    // bservations less than or equal to 1000ms, even if they also increment
    // other buckets.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\",status_code=\"200\",le=\"1000\"} 2");
    // the histogram's total count should be 2.
    assert_contains!(scrape,
        "response_latency_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\",status_code=\"200\"} 2");

    info!("client.get(/hi)");
    assert_eq!(client.get("/hi"), "good morning");

    let scrape = metrics.get("/metrics");
    // request with 40ms extra latency should fall into the 50ms bucket.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\",status_code=\"200\",le=\"50\"} 2");
    // 1000ms bucket should be incremented as well.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\",status_code=\"200\",le=\"1000\"} 3");
    // the histogram's total count should be 3.
    assert_contains!(scrape,
        "response_latency_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\",status_code=\"200\"} 3");

    info!("client.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    // 50ms bucket should be un-changed by the request with 500ms latency.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\",status_code=\"200\",le=\"50\"} 2");
    // 1000ms bucket should be incremented.
    assert_contains!(scrape,
        "response_latency_ms_bucket{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\",status_code=\"200\",le=\"1000\"} 4");
    // the histogram's total count should be 4.
    assert_contains!(scrape,
        "response_latency_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\",status_code=\"200\"} 4");
}

#[test]
fn metrics_endpoint_inbound_request_duration() {
    let _ = env_logger::try_init();

    info!("running test server");
    let srv = server::new()
        .route("/hey", "hello")
        .run();

    let ctrl = controller::new();
    let proxy = proxy::new()
        .controller(ctrl.run())
        .inbound(srv)
        .run();
    let client = client::new(proxy.inbound, "tele.test.svc.cluster.local");
    let metrics = client::http1(proxy.metrics, "localhost");

    // request with body should increment request_duration
    info!("client.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    assert_contains!(scrape,
        "request_duration_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\"} 1");

    // request without body should also increment request_duration
    info!("client.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    assert_contains!(scrape,
        "request_duration_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"inbound\"} 2");
}

#[test]
fn metrics_endpoint_outbound_request_duration() {
    let _ = env_logger::try_init();

    info!("running test server");
    let srv = server::new()
        .route("/hey", "hello")
        .run();
    let ctrl = controller::new()
        .destination("tele.test.svc.cluster.local", srv.addr)
        .run();
    let proxy = proxy::new()
        .controller(ctrl)
        .outbound(srv)
        .run();
    let client = client::new(proxy.outbound, "tele.test.svc.cluster.local");
    let metrics = client::http1(proxy.metrics, "localhost");

    info!("client.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    assert_contains!(scrape,
        "request_duration_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\"} 1");

    info!("client.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    assert_contains!(scrape,
        "request_duration_ms_count{authority=\"tele.test.svc.cluster.local\",direction=\"outbound\"} 2");
}

#[test]
fn metrics_have_no_double_commas() {
    // Test for regressions to runconduit/conduit#600.
    let _ = env_logger::try_init();

    info!("running test server");
    let inbound_srv = server::new().route("/hey", "hello").run();
    let outbound_srv = server::new().route("/hey", "hello").run();

    let ctrl = controller::new()
        .destination("tele.test.svc.cluster.local", outbound_srv.addr)
        .run();
    let proxy = proxy::new()
        .controller(ctrl)
        .inbound(inbound_srv)
        .outbound(outbound_srv)
        .run();
    let client = client::new(proxy.inbound, "tele.test.svc.cluster.local");
    let metrics = client::http1(proxy.metrics, "localhost");

    let scrape = metrics.get("/metrics");
    assert!(!scrape.contains(",,"));

    info!("inbound.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    assert!(!scrape.contains(",,"), "inbound metrics had double comma");

    let client = client::new(proxy.outbound, "tele.test.svc.cluster.local");

    info!("outbound.get(/hey)");
    assert_eq!(client.get("/hey"), "hello");

    let scrape = metrics.get("/metrics");
    assert!(!scrape.contains(",,"), "outbound metrics had double comma");
}
