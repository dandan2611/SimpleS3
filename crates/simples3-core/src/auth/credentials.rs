use uuid::Uuid;

pub fn generate_access_key_id() -> String {
    let id = Uuid::new_v4().to_string().replace("-", "");
    format!("AKID{}", &id[..16].to_uppercase())
}

pub fn generate_secret_access_key() -> String {
    let s1 = Uuid::new_v4().to_string().replace("-", "");
    let s2 = Uuid::new_v4().to_string().replace("-", "");
    format!("{}{}", &s1[..20], &s2[..20])
}
