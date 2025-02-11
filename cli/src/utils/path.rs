use super::feedback::error;

pub struct PathIter<I>
where
    I: Iterator<Item = String>,
{
    iter: I,
}

impl<I> Iterator for PathIter<I>
where
    I: Iterator<Item = String>,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

impl<I> PathIter<I>
where
    I: Iterator<Item = String>,
{
    pub fn new(iter: I) -> Self {
        Self { iter }
    }

    pub fn next_segment(&mut self) -> Result<String, String> {
        self.next().ok_or(error!("path terminates early."))
    }
}
