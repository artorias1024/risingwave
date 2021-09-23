use super::{Array, ArrayBuilder, ArrayIterator, NULL_VAL_FOR_HASH};

use crate::buffer::Bitmap;

use crate::error::Result;

use crate::types::NativeType;

use crate::array2::ArrayImpl;
use risingwave_proto::data::{Buffer as BufferProto, Buffer, Buffer_CompressionType};
use std::fmt::Debug;
use std::{
    hash::{Hash, Hasher},
    mem::size_of,
};

/// Physical type of array items. It differs from NativeType with more limited type set.
/// Specifically, it doesn't support u8/u16/u32/u64.
pub trait PrimitiveArrayItemType: NativeType {
    /// A helper to convert a primitive array to ArrayImpl.
    fn erase_array_type(arr: PrimitiveArray<Self>) -> ArrayImpl;
}

impl PrimitiveArrayItemType for i16 {
    fn erase_array_type(arr: PrimitiveArray<Self>) -> ArrayImpl {
        ArrayImpl::Int16(arr)
    }
}
impl PrimitiveArrayItemType for i32 {
    fn erase_array_type(arr: PrimitiveArray<Self>) -> ArrayImpl {
        ArrayImpl::Int32(arr)
    }
}
impl PrimitiveArrayItemType for i64 {
    fn erase_array_type(arr: PrimitiveArray<Self>) -> ArrayImpl {
        ArrayImpl::Int64(arr)
    }
}
impl PrimitiveArrayItemType for f32 {
    fn erase_array_type(arr: PrimitiveArray<Self>) -> ArrayImpl {
        ArrayImpl::Float32(arr)
    }
}
impl PrimitiveArrayItemType for f64 {
    fn erase_array_type(arr: PrimitiveArray<Self>) -> ArrayImpl {
        ArrayImpl::Float64(arr)
    }
}

/// `PrimitiveArray` is a collection of primitive types, such as `i32`, `f32`.
#[derive(Debug)]
pub struct PrimitiveArray<T: PrimitiveArrayItemType> {
    bitmap: Bitmap,
    data: Vec<T>,
}

impl<T: PrimitiveArrayItemType> PrimitiveArray<T> {
    pub fn from_slice(data: &[Option<T>]) -> Result<Self> {
        let mut builder = <Self as Array>::Builder::new(data.len())?;
        for i in data {
            builder.append(*i)?;
        }
        builder.finish()
    }
}

impl<T: PrimitiveArrayItemType> Array for PrimitiveArray<T> {
    type Builder = PrimitiveArrayBuilder<T>;
    type RefItem<'a> = T;
    type OwnedItem = T;
    type Iter<'a> = ArrayIterator<'a, Self>;

    fn value_at(&self, idx: usize) -> Option<T> {
        if !self.is_null(idx) {
            Some(self.data[idx])
        } else {
            None
        }
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn iter(&self) -> Self::Iter<'_> {
        ArrayIterator::new(self)
    }

    fn to_protobuf(&self) -> crate::error::Result<Vec<Buffer>> {
        let values = {
            let mut output_buffer = Vec::<u8>::with_capacity(self.len() * size_of::<T>());

            for v in self.iter() {
                v.map(|node| node.to_protobuf(&mut output_buffer));
            }

            let mut b = BufferProto::new();
            b.set_compression(Buffer_CompressionType::NONE);
            b.set_body(output_buffer);
            b
        };
        Ok(vec![values])
    }

    fn null_bitmap(&self) -> &Bitmap {
        &self.bitmap
    }

    #[inline(always)]
    fn hash_at<H: Hasher>(&self, idx: usize, state: &mut H) {
        if !self.is_null(idx) {
            self.data[idx].hash_wrapper(state);
        } else {
            NULL_VAL_FOR_HASH.hash(state);
        }
    }
}

/// `PrimitiveArrayBuilder` constructs a `PrimitiveArray` from `Option<Primitive>`.
pub struct PrimitiveArrayBuilder<T: PrimitiveArrayItemType> {
    bitmap: Vec<bool>,
    data: Vec<T>,
}

impl<T: PrimitiveArrayItemType> ArrayBuilder for PrimitiveArrayBuilder<T> {
    type ArrayType = PrimitiveArray<T>;

    fn new(capacity: usize) -> Result<Self> {
        Ok(Self {
            bitmap: Vec::with_capacity(capacity),
            data: Vec::with_capacity(capacity),
        })
    }

    fn append(&mut self, value: Option<T>) -> Result<()> {
        match value {
            Some(x) => {
                self.bitmap.push(true);
                self.data.push(x);
            }
            None => {
                self.bitmap.push(false);
                self.data.push(T::default());
            }
        }
        Ok(())
    }

    fn append_array(&mut self, other: &PrimitiveArray<T>) -> Result<()> {
        self.bitmap.extend(other.bitmap.iter());
        self.data.extend_from_slice(&other.data);
        Ok(())
    }

    fn finish(self) -> Result<PrimitiveArray<T>> {
        Ok(PrimitiveArray {
            bitmap: Bitmap::from_vec(self.bitmap)?,
            data: self.data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn helper_test_builder<T: PrimitiveArrayItemType>(
        data: Vec<Option<T>>,
    ) -> Result<PrimitiveArray<T>> {
        let mut builder = PrimitiveArrayBuilder::<T>::new(data.len())?;
        for d in data {
            builder.append(d)?;
        }
        builder.finish()
    }

    #[test]
    fn test_i16_builder() {
        let arr = helper_test_builder::<i16>(
            (0..1000)
                .map(|x| if x % 2 == 0 { None } else { Some(x) })
                .collect(),
        )
        .unwrap();
        if !matches!(ArrayImpl::from(arr), ArrayImpl::Int16(_)) {
            unreachable!()
        }
    }

    #[test]
    fn test_i32_builder() {
        let arr = helper_test_builder::<i32>(
            (0..1000)
                .map(|x| if x % 2 == 0 { None } else { Some(x) })
                .collect(),
        )
        .unwrap();
        if !matches!(ArrayImpl::from(arr), ArrayImpl::Int32(_)) {
            unreachable!()
        }
    }

    #[test]
    fn test_i64_builder() {
        let arr = helper_test_builder::<i64>(
            (0..1000)
                .map(|x| if x % 2 == 0 { None } else { Some(x) })
                .collect(),
        )
        .unwrap();
        if !matches!(ArrayImpl::from(arr), ArrayImpl::Int64(_)) {
            unreachable!()
        }
    }

    #[test]
    fn test_f32_builder() {
        let arr = helper_test_builder::<f32>(
            (0..1000)
                .map(|x| if x % 2 == 0 { None } else { Some(x as f32) })
                .collect(),
        )
        .unwrap();
        if !matches!(ArrayImpl::from(arr), ArrayImpl::Float32(_)) {
            unreachable!()
        }
    }

    #[test]
    fn test_f64_builder() {
        let arr = helper_test_builder::<f64>(
            (0..1000)
                .map(|x| if x % 2 == 0 { None } else { Some(x as f64) })
                .collect(),
        )
        .unwrap();
        if !matches!(ArrayImpl::from(arr), ArrayImpl::Float64(_)) {
            unreachable!()
        }
    }
}
