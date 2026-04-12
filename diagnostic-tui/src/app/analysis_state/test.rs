#[test]
fn test_normalized() {
    let s = "Error creating system vault for account (YFIYEKAI6NGMZOW6AR3CQBCPRE): FetchDataError(FetchError(<unknown reason>, code: HttpStatus(400), Session ID: Some(Session ID: FPQYCQJLX5EDNL4CN3UW65SQOA)))";
    let normalized = super::normalize(s);
    assert_eq!(
        "Error creating system vault for account (<ID>): FetchDataError(FetchError(<unknown reason>, code: HttpStatus(400), Session ID: Some(Session ID: <ID>)))",
        normalized,
    );
}
