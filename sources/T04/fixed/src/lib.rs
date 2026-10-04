#![allow(dead_code)]
#![no_std]

extern crate alloc;

use alloc::{
    alloc::{alloc, Layout, dealloc, handle_alloc_error},
    fmt, fmt::{Debug, Display, Formatter},
    slice::{self, },
};

use core::{
    mem::size_of, 
    ops::{Index, IndexMut},
    iter::{IntoIterator, Iterator},
    clone::Clone,
};

/// An fixed-sized array allocated in the heap
/// of any arbitrary type, and whose
/// size needn't be known at compile time.
pub struct Caja<T> {
    /// Length of the array
    length : usize,
    data   : *mut T,
}
/// An iterator for &Caja
pub struct CajaIterator<'a, T> {
    caja: &'a Caja<T>,
    index: usize,
}
/// An iterator for &mut Caja
pub struct CajaIteratorMut<'a, T> {
    caja: &'a mut Caja<T>,
    index: usize,
}

impl<T> Caja<T> {
    /// If successful, allocates size * size_of::<T>() bytes into the heap,
    /// resulting in an uninitialized array that is used by Caja.
    ///
    /// This functions is unsafe only because the user has to ensure
    /// that the underlying elements are well defined by themselves
    ///
    /// Note: size must be non zero to obtain a valid pointer.
    pub unsafe fn new_uninitialized(size : usize) -> Self {
        if size == 0 {
            return Self {
                length : 0,
                data   : core::ptr::null_mut()
            };
        }

        // Create a layout for the allocation
        let lay = Layout::array::<T>(size).unwrap();

        // Check that the allocation was successful
        let ptr = alloc(lay) as *mut T;
        if ptr.is_null() {
            handle_alloc_error(lay);
        }

        Self {
            length : size,
            data   : ptr,
        }
    }

    /// If successful, allocates size * size_of::<T>() bytes into the heap,
    /// and then initializes each byte with 0.
    ///
    /// This functions is unsafe only because the user has to ensure
    /// that the underlying elements are well defined by themselves
    ///
    /// Note: size must be non zero to obtain a valid pointer.
    pub unsafe fn new_zeroed(size : usize) -> Self {
        let c = Self::new_uninitialized(size);

        for i in 0..size*size_of::<T>() {
            *(c.data as *mut u8).add(i) = 0;
        }

        c
    }

    /// Returns the underlying (mut) pointer in Caja
    #[inline(always)]
    pub fn as_mut_ptr(&self) -> *mut T {
        self.data
    }

    /// Returns the underlying pointer in Caja
    #[inline(always)]
    pub fn as_ptr(&self) -> *const T {
        self.data
    }

    /// Returns the length of the array
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.length
    }

    /// Returns a slice of the array
    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {unsafe{
        slice::from_raw_parts(self.data, self.length)
    }}

    /// Returns a mutable sliice of the array
    #[inline(always)]
    pub fn as_mut_slice(&self) -> &mut [T] {unsafe{
        slice::from_raw_parts_mut(self.data, self.length)
    }}

    ///  Index into the array. This function does NOT do bounds checking
    #[inline(always)]
    pub unsafe fn unchecked_index<'a>(&self, index : usize) -> &'a T {
        unsafe { self.data.add(index).as_ref().unwrap() }
    }

    ///  Index into the array (mutably). This function does NOT do bounds checking
    #[inline(always)]
    pub unsafe fn unchecked_index_mut<'a>(&mut self, index : usize) -> &'a mut T {
        unsafe {self.data.add(index).as_mut().unwrap() }
    }
}
impl<T> Drop for Caja<T> {
    fn drop(&mut self) {unsafe{
        dealloc(
            self.data as *mut u8,
            Layout::array::<T>(self.length).unwrap()
        );
    };}
}
impl<T> Index<usize> for Caja<T> {
    type Output = T;

    #[inline(always)]
    fn index(&self, index : usize) -> &Self::Output {
        assert!(self.length > index);
        unsafe { self.data.add(index).as_ref().unwrap() }
    }
}
impl<T> IndexMut<usize> for Caja<T> {
    #[inline(always)]
    fn index_mut(&mut self, index : usize) -> &mut Self::Output {
        assert!(self.length > index);
        unsafe { self.data.add(index).as_mut().unwrap() }
    }
}
impl<'a, T> Iterator for CajaIterator<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.caja.length > self.index {
            let r = unsafe {
                self.caja.unchecked_index(self.index)
            };
            self.index += 1;
            Some(r)
        } else { None }
    }
}
impl<'a, T> Iterator for CajaIteratorMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.caja.length > self.index {
            let r = unsafe {
                self.caja.unchecked_index_mut(self.index)
            };
            self.index += 1;
            Some(r)
        } else { None }
    }
}

impl<'a, T> IntoIterator for &'a Caja<T> {    
    type Item = &'a T;
    type IntoIter = CajaIterator<'a, T>;
    
    fn into_iter(self) -> Self::IntoIter {
        CajaIterator {
            caja: self,
            index: 0,
        }
    }
}
impl<'a, T> IntoIterator for &'a mut Caja<T> {    
    type Item = &'a mut T;
    type IntoIter = CajaIteratorMut<'a, T>;
    
    fn into_iter(self) -> Self::IntoIter {
        CajaIteratorMut {
            caja: self,
            index: 0,
        }
    }
}
impl<T : Display> Display for Caja<T> {
    fn fmt(&self, format : &mut Formatter<'_>) -> fmt::Result {
        let mut res = write!(format, "Length : {}\nData : [ ", self.length);
        
        for i in 0..self.length-1 {
            if res.is_err() {
                return res;
            }

            res = write!(format, "{}, ", self[i]);
        }
        res = write!(format, "{} ]\n", self[self.length-1]);

        res
    }
}
impl<T : Debug> Debug for Caja<T> {
    fn fmt(&self, format : &mut Formatter<'_>) -> fmt::Result {unsafe{
        format.debug_struct("Caja")
            .field("length", &self.length)
            .field("data", &self.data)
            .field("data as an array", &slice::from_raw_parts(self.data, self.length))
            .finish()
    }}
}
impl<T : Copy> Caja<T> { 
    /// If successful, allocates an array of type 'T' and size 'size' into 
    /// the heap, and initializes each element with 'default'.
    /// T must implement Copy for this to work.
    /// This function will panic in the same conditions alloc::alloc(Layout) will
    /// (so the same conditions for Box<T>, Vec<T>, etc).
    ///
    /// Note: size must be non zero to obtain a valid pointer.
    pub fn new(size : usize, default : T) -> Self {
        let mut c = unsafe { Self::new_uninitialized(size) };

        for i in &mut c {
            *i = default;
        }

        c
    }
}

impl<T: Copy> Clone for Caja<T> {
    fn clone(&self) -> Self {
        let mut r = unsafe { Self::new_uninitialized(self.length) };

        for i in 0..self.length {
            r[i] = self[i];
        }

        r
    }
}

impl<T : Copy> From<&[T]> for Caja<T> {
    /// Creates a Caja from a slice, copying the data into
    /// a new buffer in the heap.
    ///
    /// Because this functions creates a  new caja, it will panic 
    /// under the same conditions as the new variations
    /// (so the same conditions for Box<T>, Vec<T>, etc).
    fn from(frm : &[T]) -> Self {
        let mut ret = unsafe { Self::new_uninitialized(frm.len()) };
        
        for i in 0..frm.len() {
            ret[i] = frm[i];
        }

        ret
    }
}
