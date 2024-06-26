use std::fmt::Debug;

#[derive(Debug)]
pub struct VecMap<T: Debug> {
  inner: Vec<Option<T>>,
}

impl<T: Debug> Default for VecMap<T> {
  fn default() -> Self {
    Self { inner: vec![] }
  }
}

impl<T: Debug> VecMap<T> {
  pub fn insert(&mut self, index: usize, v: T) {
    if index < self.inner.len() {
      self.inner[index] = Some(v);
    } else {
      while self.inner.len() < index {
        self.inner.push(None);
      }
      self.inner.push(Some(v));
    }
  }

  /// # Panic
  /// the function would panic if bounds check failed or value does not exists
  pub fn get(&self, index: usize) -> &T {
    self.inner[index].as_ref().expect("should have value")
  }

  /// # Panic
  /// the function would panic if bounds check failed or value does not exists
  pub fn get_mut(&mut self, index: usize) -> &mut T {
    self.inner[index].as_mut().expect("should have value")
  }

  pub fn try_get(&self, index: usize) -> Option<&T> {
    self.inner.get(index).and_then(|item| item.as_ref())
  }

  //  pub fn try_get_mut(&mut self, index: usize) -> Option<&mut T> {
  //    self.inner.get_mut(index).and_then(|item| item.as_mut())
  //  }
}
