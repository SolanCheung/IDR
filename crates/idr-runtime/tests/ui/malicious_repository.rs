use idr_runtime::IdrTransactionalRepositoryV1;

fn main() {
    fn accept_repository<T: IdrTransactionalRepositoryV1>() {}
    let _ = accept_repository::<()>;
}
