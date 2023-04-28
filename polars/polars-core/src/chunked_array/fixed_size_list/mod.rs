//! Special fixed-size-list utility methods

use crate::chunked_array::Settings;
use crate::prelude::*;

impl FixedSizeListChunked {
    #[cfg(feature = "private")]
    pub fn set_fast_explode(&mut self) {
        self.bit_settings.insert(Settings::FAST_EXPLODE_LIST)
    }
    pub(crate) fn unset_fast_explode(&mut self) {
        self.bit_settings.remove(Settings::FAST_EXPLODE_LIST)
    }

    pub fn _can_fast_explode(&self) -> bool {
        self.bit_settings.contains(Settings::FAST_EXPLODE_LIST)
    }

    pub(crate) fn is_nested(&self) -> bool {
        match self.dtype() {
            DataType::List(inner) => matches!(&**inner, DataType::List(_)),
            _ => unreachable!(),
        }
    }

    pub fn to_logical(&mut self, inner_dtype: DataType) {
        debug_assert_eq!(inner_dtype.to_physical(), self.inner_dtype());
        let fld = Arc::make_mut(&mut self.field);
        fld.coerce(DataType::List(Box::new(inner_dtype)))
    }

    /// Get the inner values as `Series`, ignoring the list offsets.
    pub fn get_inner(&self) -> Series {
        let ca = self.rechunk();
        let inner_dtype = self.inner_dtype().to_arrow();
        let arr = ca.downcast_iter().next().unwrap();
        unsafe {
            Series::try_from_arrow_unchecked(
                self.name(),
                vec![(*arr.values()).clone()],
                &inner_dtype,
            )
            .unwrap()
        }
    }

    /// Ignore the list indices and apply `func` to the inner type as `Series`.
    pub fn apply_to_inner(
        &self,
        func: &dyn Fn(Series) -> PolarsResult<Series>,
    ) -> PolarsResult<FixedSizeListChunked> {
        // generated Series will have wrong length otherwise.
        let ca = self.rechunk();
        let inner_dtype = self.inner_dtype().to_arrow();

        let chunks = ca.downcast_iter().map(|arr| {
            let elements = unsafe { Series::try_from_arrow_unchecked(self.name(), vec![(*arr.values()).clone()], &inner_dtype).unwrap() } ;

            let expected_len = elements.len();
            let out: Series = func(elements)?;
            polars_ensure!(
                out.len() == expected_len,
                ComputeError: "the function should apply element-wise, it removed elements instead"
            );
            let out = out.rechunk();
            let values = out.chunks()[0].clone();

            let inner_dtype = FixedSizeListArray::default_datatype(out.dtype().to_arrow(), ca.inner_size());
            let arr = FixedSizeListArray::new(
                inner_dtype,
                values,
                arr.validity().cloned(),
            );
            Ok(Box::new(arr) as ArrayRef)
        }).collect::<PolarsResult<Vec<_>>>()?;

        unsafe { Ok(FixedSizeListChunked::from_chunks(self.name(), chunks)) }
    }
}
