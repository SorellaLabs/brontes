use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use eyre::WrapErr;
use std::{convert::Infallible, net::SocketAddr};

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};

async fn start_db_write_switch(switch: Arc<AtomicBool>) -> eyre::Result<()> {

    let make_svc = make_service_fn(move |_| {
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                req

                let mut metrics_render = handle.render();

                let mut buffer = Vec::new();
                let encoder = TextEncoder::new();
                // Gather the metrics.
                let metric_families = prometheus::gather();
                // Encode them to send.
                encoder.encode(&metric_families, &mut buffer).unwrap();
                metrics_render += &String::from_utf8(buffer.clone()).unwrap();

                async move { Ok::<_, Infallible>(Response::new(Body::from(metrics_render))) }
            }))
        }
    });
    let server = Server::try_bind(&listen_addr)
        .wrap_err("Could not bind to address")?
        .serve(make_svc);

    tokio::spawn(async move { server.await.expect("Metrics endpoint crashed") });

    Ok(())
}

