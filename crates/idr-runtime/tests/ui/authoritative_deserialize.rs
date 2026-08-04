use idr_runtime::AuthoritativeRecordV1;

fn main() {
    let _: AuthoritativeRecordV1 = serde_json::from_str("{}").unwrap();
}
