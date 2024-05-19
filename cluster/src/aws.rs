pub(crate) async fn load_aws_config(region: &Option<String>) -> aws_config::SdkConfig {
    use aws_types::region::Region;
    if let Some(ref region) = region {
        aws_config::from_env()
            .region(Region::new(region.clone()))
            .load()
            .await
    } else {
        aws_config::from_env().load().await
    }
}
