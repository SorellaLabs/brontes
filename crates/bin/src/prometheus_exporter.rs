use eyre::{Result, WrapErr};
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use metrics_util::layers::{PrefixLayer, Stack};
use metrics_process::Collector;
use std::{convert::Infallible, net::SocketAddr};
use std::sync::Arc;

/// Installs Prometheus as the metrics recorder and serves it over HTTP.
pub async fn initialize(listen_addr: SocketAddr, prometheus_collector: Collector) -> eyre::Result<()> {

    let clone_collector = prometheus_collector.clone();
    let collector: Box<dyn Fn() + Send + Sync> = Box::new(move || clone_collector.collect());

    let recorder = PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();

    // Start endpoint
    start_endpoint(listen_addr, handle, Arc::new(move || collector())).await.wrap_err("Could not start Prometheus endpoint")?;

    // Build metrics stack
    Stack::new(recorder)
        .push(PrefixLayer::new("mev-tracing"))
        .install()
        .wrap_err("Couldn't set metrics recorder.")?;

    prometheus_collector.collect();

    Ok(())
}

/// Starts an endpoint at the given address to serve Prometheus metrics.
async fn start_endpoint<T: 'static + Fn() + Sync + Send>(listen_addr: SocketAddr, handle: PrometheusHandle, collector: Arc<T>) -> Result<()> {
    let make_svc = make_service_fn(move |_| {
        let handle = handle.clone();
        let collector = Arc::clone(&collector);
        async move {
            Ok::<_, Infallible>(service_fn(move |_: Request<Body>| {
                (collector)();
                let metrics = handle.render();
                async move { Ok::<_, Infallible>(Response::new(Body::from(metrics))) }
            }))
        }
    });
    let server =
        Server::try_bind(&listen_addr).wrap_err("Could not bind to address")?.serve(make_svc);

    tokio::spawn(async move { server.await.expect("Metrics endpoint crashed") });

    Ok(())
}