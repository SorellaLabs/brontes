async fn run_until_ctrl_c<F, E>(fut: F) -> Result<(), E>
where
    F: Future<Output = Result<(), E>>,
    E: Send + Sync + 'static + From<std::io::Error>,
{
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut stream = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        let sigterm = stream.recv();
        pin_mut!(sigterm, ctrl_c, fut);

        tokio::select! {
            _ = ctrl_c => {
                trace!(target: "reth::cli",  "Received ctrl-c");
            },
            _ = sigterm => {
                trace!(target: "reth::cli",  "Received SIGTERM");
            },
            res = fut => res?,
        }
    }

    #[cfg(not(unix))]
    {
        pin_mut!(ctrl_c, fut);

        tokio::select! {
            _ = ctrl_c => {
                trace!(target: "reth::cli",  "Received ctrl-c");
            },
            res = fut => res?,
        }
    }

    Ok(())
}
