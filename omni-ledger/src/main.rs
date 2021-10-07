use clap::Clap;
use omni::Identity;
use ring::signature::KeyPair;
use std::path::PathBuf;
use tracing_subscriber::filter::LevelFilter;

#[derive(Debug, Clap)]
struct Opt {
    // Pem file to use for the key. If omitted, the anonymous identity will be used.
    #[clap(long)]
    pem: Option<PathBuf>,

    /// Increase output logging verbosity to DEBUG level.
    #[clap(short, long)]
    verbose: bool,

    /// Suppress all output logging (overrides --verbose).
    #[clap(short, long)]
    quiet: bool,
}

fn to_der(key: Vec<u8>) -> Vec<u8> {
    use simple_asn1::{
        oid, to_der,
        ASN1Block::{BitString, ObjectIdentifier, Sequence},
    };

    let public_key = key;
    let id_ed25519 = oid!(1, 3, 101, 112);
    let algorithm = Sequence(0, vec![ObjectIdentifier(0, id_ed25519)]);
    let subject_public_key = BitString(0, public_key.len() * 8, public_key);
    let subject_public_key_info = Sequence(0, vec![algorithm, subject_public_key]);
    to_der(&subject_public_key_info).unwrap()
}

fn main() {
    let opt: Opt = Opt::parse();
    let log_level = if opt.quiet {
        LevelFilter::OFF
    } else if opt.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    tracing_subscriber::fmt().with_max_level(log_level).init();

    let mut cose_sign = cose::sign::CoseSign::new();
    cose_sign.header.alg(cose::algs::EDDSA, true, false);

    if let Some(pem_path) = opt.pem {
        let bytes = std::fs::read(pem_path).unwrap();
        let content = pem::parse(bytes).unwrap();
        let keypair =
            ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&content.contents).unwrap();
        // let key = ed25519_dalek::Keypair::from_bytes(&content.contents).unwrap();
        // let keypair = ed25519_dalek::Keypair::from_bytes(&bytes).unwrap();
        // hex::decode(concat!(
        //     "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
        //     "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
        // ))
        // .unwrap()
        // .as_slice(),
        // )
        // .unwrap();

        use cose::{algs, keys};

        // let mut key = keys::CoseKey::new();
        // key.kid(Identity::public_key(to_der(keypair.public_key().as_ref().to_vec())).to_vec());
        // key.kty(keys::EC2);
        // key.alg(algs::EDDSA);
        // key.crv(keys::ED25519);
        //
        // key.x(keypair.public_key().to_bytes().to_vec());
        // key.d(keypair.secret_key().to_bytes().to_vec());
        // key.key_ops(vec![keys::KEY_OPS_SIGN]);
        //
        // cose_sign.key(&key).unwrap();
        // cose_sign.gen_signature(None).unwrap();
    } else {
        cose_sign
            .header
            .kid(Identity::anonymous().to_vec(), true, false);
    };

    cose_sign.payload(vec![0, 1, 2, 3]);
    cose_sign.encode(true).unwrap();
    let bytes = cose_sign.bytes;

    eprintln!("message:");
    eprintln!("{}", hex::encode(&bytes));
    println!("{}", base64::encode(bytes));
}
