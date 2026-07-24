#[path = "../src/proxy.rs"]
mod proxy;
#[test]
fn frames_are_bounded_and_round_trip() {
    let mut b = Vec::new();
    proxy::write_frame(&mut b, b"{} ").unwrap();
    let mut s = &b[..];
    assert_eq!(proxy::read_frame(&mut s).unwrap(), b"{} ");
    let huge = vec![0; proxy::MAX_FRAME + 1];
    assert!(matches!(
        proxy::write_frame(&mut Vec::new(), &huge),
        Err(proxy::ProxyError::FrameTooLarge)
    ));
}
#[test]
fn absent_daemon_is_typed() {
    let e = proxy::connect(std::path::Path::new("/tmp/no-agent-graph.sock")).unwrap_err();
    assert!(matches!(e, proxy::ProxyError::DaemonUnavailable));
}
