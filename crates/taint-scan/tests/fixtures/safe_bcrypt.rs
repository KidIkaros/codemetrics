// @sensitive
let password = load_password();
fn hash_password() { let h = bcrypt_hash(&password); store(h); }
