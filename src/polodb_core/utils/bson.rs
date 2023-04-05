/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
use std::cmp::Ordering;
use std::io::Write;
use bson::Bson;
use bson::spec::ElementType;
use byteorder::{BigEndian, WriteBytesExt};
use bson::ser::Error as BsonErr;
use bson::ser::Result as BsonResult;
use crate::{DbErr, DbResult};

pub fn stacked_key<'a, T: IntoIterator<Item = &'a Bson>>(keys: T) -> DbResult<Vec<u8>> {
    let mut result = Vec::<u8>::new();

    for key in keys {
        stacked_key_bytes(&mut result, key)?;
    }

    Ok(result)
}

pub fn stacked_key_bytes<W: Write>(writer: &mut W, key: &Bson) -> DbResult<()> {
    match key {
        Bson::Double(dbl) => {
            writer.write_u8(ElementType::Double as u8)?;
            writer.write_f64::<BigEndian>(*dbl)?;
        }
        Bson::String(str) => {
            writer.write_u8(ElementType::String as u8)?;

            writer.write_all(str.as_bytes())?;

            writer.write_u8(0)?;
        }
        Bson::Boolean(bl) => {
            writer.write_u8(ElementType::Boolean as u8)?;

            writer.write_u8(*bl as u8)?;
        }
        Bson::Null => {
            writer.write_u8(ElementType::Null as u8)?;
        }
        Bson::Int32(i32) => {
            writer.write_u8(ElementType::Int32 as u8)?;

            writer.write_i32::<BigEndian>(*i32)?;
        }
        Bson::Int64(i64) => {
            writer.write_u8(ElementType::Int64 as u8)?;

            writer.write_i64::<BigEndian>(*i64)?;
        }
        Bson::Timestamp(ts) => {
            writer.write_u8(ElementType::Timestamp as u8)?;

            let u64 = ((ts.time as u64) << 32) | (ts.increment as u64);

            writer.write_u64::<BigEndian>(u64)?;
        }
        Bson::ObjectId(oid) => {
            writer.write_u8(ElementType::ObjectId as u8)?;

            let bytes = oid.bytes();
            writer.write_all(&bytes)?;
        }
        Bson::DateTime(dt) => {
            writer.write_u8(ElementType::DateTime as u8)?;

            let t = dt.timestamp_millis();

            writer.write_i64::<BigEndian>(t)?;
        }
        Bson::Symbol(str) => {
            writer.write_u8(ElementType::Symbol as u8)?;

            writer.write_all(str.as_bytes())?;

            writer.write_u8(0)?;
        }
        Bson::Decimal128(dcl) => {
            writer.write_u8(ElementType::Decimal128 as u8)?;

            let bytes = dcl.bytes();

            writer.write_all(&bytes)?;
        }
        Bson::Undefined => {
            writer.write_u8(ElementType::Undefined as u8)?;
        }

        _ => {
            let val = format!("{:?}", key);
            return Err(DbErr::NotAValidKeyType(val))
        }
    }

    Ok(())
}

pub fn value_cmp(a: &Bson, b: &Bson) -> BsonResult<Ordering> {
    match (a, b) {
        (Bson::Null, Bson::Null) => Ok(Ordering::Equal),
        (Bson::Undefined, Bson::Undefined) => Ok(Ordering::Equal),
        (Bson::DateTime(d1), Bson::DateTime(d2)) => Ok(d1.cmp(d2)),
        (Bson::Boolean(b1), Bson::Boolean(b2)) => Ok(b1.cmp(b2)),
        (Bson::Int64(i1), Bson::Int64(i2)) => Ok(i1.cmp(i2)),
        (Bson::Int32(i1), Bson::Int32(i2)) => Ok(i1.cmp(i2)),
        (Bson::Int64(i1), Bson::Int32(i2)) => {
            let i2_64 = *i2 as i64;
            Ok(i1.cmp(&i2_64))
        },
        (Bson::Int32(i1), Bson::Int64(i2)) => {
            let i1_64 = *i1 as i64;
            Ok(i1_64.cmp(i2))
        },
        (Bson::Double(d1), Bson::Double(d2)) => Ok(d1.total_cmp(d2)),
        (Bson::Double(d1), Bson::Int32(d2)) => {
            let f = *d2 as f64;
            Ok(d1.total_cmp(&f))
        },
        (Bson::Double(d1), Bson::Int64(d2)) => {
            let f = *d2 as f64;
            Ok(d1.total_cmp(&f))
        },
        (Bson::Int32(i1), Bson::Double(d2)) => {
            let f = *i1 as f64;
            Ok(f.total_cmp(d2))
        }
        (Bson::Int64(i1), Bson::Double(d2)) => {
            let f = *i1 as f64;
            Ok(f.total_cmp(d2))
        }
        (Bson::Binary(b1), Bson::Binary(b2)) => Ok(b1.bytes.cmp(&b2.bytes)),
        (Bson::String(str1), Bson::String(str2)) => Ok(str1.cmp(str2)),
        (Bson::ObjectId(oid1), Bson::ObjectId(oid2)) => Ok(oid1.cmp(oid2)),
        _ => {
            // compare the numeric type
            let a_type = a.element_type() as u8;
            let b_type = b.element_type() as u8;
            if a_type != b_type {
                return Ok(a_type.cmp(&b_type));
            }

            Err(BsonErr::InvalidCString("Unsupported types".to_string()))
        },
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use bson::Bson;
    use crate::utils::bson::value_cmp;

    #[test]
    fn test_value_cmp() {
        assert_eq!(value_cmp(&Bson::Int32(2), &Bson::Int64(3)).unwrap(), Ordering::Less);
        assert_eq!(value_cmp(&Bson::Int32(2), &Bson::Int64(1)).unwrap(), Ordering::Greater);
        assert_eq!(value_cmp(&Bson::Int32(1), &Bson::Int64(1)).unwrap(), Ordering::Equal);
        assert_eq!(value_cmp(&Bson::Int64(2), &Bson::Int32(3)).unwrap(), Ordering::Less);
        assert_eq!(value_cmp(&Bson::Int64(2), &Bson::Int32(1)).unwrap(), Ordering::Greater);
        assert_eq!(value_cmp(&Bson::Int64(1), &Bson::Int32(1)).unwrap(), Ordering::Equal);
    }

}
