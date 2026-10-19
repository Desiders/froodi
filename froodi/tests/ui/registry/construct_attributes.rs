use froodi::utils::thread_safety::RcThreadSafety;

#[derive(froodi::Construct)]
struct Unsupported {
    #[di(optional)]
    value: RcThreadSafety<u32>,
}

#[derive(froodi::Construct)]
struct Conflicting {
    #[di(inject, inject_transient)]
    value: RcThreadSafety<u32>,
}

#[derive(froodi::Construct)]
struct Repeated {
    #[di(inject)]
    #[di(inject_transient)]
    value: RcThreadSafety<u32>,
}

#[derive(froodi::Construct)]
struct Arguments {
    #[di(inject = true)]
    value: RcThreadSafety<u32>,
}

#[derive(froodi::Construct)]
#[di(scope = App)]
struct RegistrationPolicy;

#[derive(froodi::Construct)]
struct FieldPolicy {
    #[di(config = ())]
    value: RcThreadSafety<u32>,
}

fn main() {}
