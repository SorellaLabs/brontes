#[macro_export]
macro_rules! implement_table_value_codecs_with_zc {
    ($table_value:ident) => {
        impl alloy_rlp::Encodable for $table_value {
            fn encode(&self, out: &mut dyn bytes::BufMut) {
                let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

                out.put_slice(&encoded)
            }
        }

        impl alloy_rlp::Decodable for $table_value {
            fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
                let archived: &paste::paste!([<Archived $table_value>]) =
                unsafe { rkyv::archived_root::<Self>(&buf[..]) };


                let this = rkyv::Deserialize::deserialize(archived, &mut rkyv::Infallible).unwrap();

                Ok(this)
            }
        }

        impl reth_db::table::Compress for $table_value {
            type Compressed = Vec<u8>;

            fn compress_to_buf<B: alloy_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B)
            {
                let mut encoded = Vec::new();
                alloy_rlp::Encodable::encode(&self, &mut encoded);
                let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

                buf.put_slice(&encoded_compressed);
            }
        }

        impl reth_db::table::Decompress for $table_value {
            fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
                let binding = value.as_ref().to_vec();

                let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
                let buf = &mut encoded_decompressed.as_slice();

                alloy_rlp::Decodable::decode(buf).map_err(|_| reth_db::DatabaseError::Decode)
            }
        }
    };
}
