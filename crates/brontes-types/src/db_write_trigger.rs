use std::{
    // convert::Infallible,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    task::Poll,
    time::Duration,
};

// use eyre::WrapErr;
use futures::Stream;
// use hyper::{body, service::service_fn, Request, Response};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{interval, Interval},
};

// const TRIGGER_ADDRESS: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 54321);

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

pub async fn start_hr_monitor(tx: Sender<()>) -> eyre::Result<()> {
    // let make_svc = service_fn(move |_| {
    //     let s = tx.clone();
    //     async move {
    //         let s = s.clone();
    //         Ok::<_, Infallible>(service_fn(move |_: Request<body::Incoming>| {
    //             s.try_send(()).unwrap();
    //             async move { Ok::<_, Infallible>(Response::default()) }
    //         }))
    //     }
    // });

    // let server = Server::try_bind(&TRIGGER_ADDRESS)
    //     .wrap_err("Could not bind to address")?
    //     .serve(make_svc);

    // tokio::spawn(async move { server.await.expect("Metrics endpoint crashed") });

    Ok(())
}

pub struct HeartRateMonitor {
    pub timeout: Interval,
    pub rx: Receiver<()>,
}

impl HeartRateMonitor {
    pub fn new(timeout: Duration, rx: Receiver<()>) -> Self {
        tracing::info!("started hr monitor");
        Self { timeout: interval(timeout), rx }
    }
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
                tracing::debug!("got heartbeat");
                return Poll::Ready(Some(true));
            }
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Pending => {}
        }

        if self.timeout.poll_tick(cx).is_ready() {
            tracing::debug!("disconnect detected, starting backup");
            return Poll::Ready(Some(false));
        }

        Poll::Pending
    }
}
