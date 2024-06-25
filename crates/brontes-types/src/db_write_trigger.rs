use std::{
    convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll},
    time::Duration,
};

use bytes::Buf;
use eyre::WrapErr;
use futures::Stream;
use hyper::{
    body::HttpBody,
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server, StatusCode,
};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::Interval,
};

const TRIGGER_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 54321);

#[allow(unreachable_code)]
pub async fn backup_server_heartbeat(url: String, ping_rate: Duration) {
    tokio::spawn(async move {
        let client = reqwest::Client::new();

        let mut interval = tokio::time::interval(ping_rate);
        loop {
            interval.tick().await;
            client
                .post(&url)
                .body(vec![0u8])
                .send()
                .await?
                .error_for_status()?;
        }

        eyre::Ok(())
    });
}

pub async fn start_db_write_switch(switch: Arc<AtomicBool>) -> eyre::Result<()> {
    let make_svc = make_service_fn(move |_| {
        let s = switch.clone();
        async move {
            let s = s.clone();
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let s = s.clone();
                async move {
                    let mut body = req.collect().await.unwrap().to_bytes();

                    if body.len() == 1 {
                        let res = body.get_u8() == 1;
                        s.store(res, Ordering::SeqCst);
                        tracing::info!(write=%res,"db writer set");
                        Ok::<_, Infallible>(Response::new(Body::from("")))
                    } else {
                        let mut res = Response::new(Body::from(""));
                        *res.status_mut() = StatusCode::BAD_REQUEST;
                        Ok(res)
                    }
                }
            }))
        }
    });

    let server = Server::try_bind(&TRIGGER_ADDRESS)
        .wrap_err("Could not bind to address")?
        .serve(make_svc);

    tokio::spawn(async move { server.await.expect("Metrics endpoint crashed") });

    Ok(())
}

pub struct HeartRateMonitor {
    pub timeout: Interval,
    pub rx:      Receiver<()>,
}

impl Stream for HeartRateMonitor {
    type Item = bool;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.rx.poll_recv(cx) {
            // reset timeout and reschedule
            Poll::Ready(Some(_)) => {
                self.timeout.reset();
                cx.waker().wake_by_ref();
                return Poll::Ready(Some(true))
            }
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Pending => {}
        }

        if self.timeout.poll_tick(cx).is_ready() {
            return Poll::Ready(Some(false))
        }

        Poll::Pending
    }
}
