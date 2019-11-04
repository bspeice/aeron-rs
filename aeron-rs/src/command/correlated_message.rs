use crate::command::flyweight::Flyweight;
use crate::concurrent::AtomicBuffer;

pub struct CorrelatedMessageDefn {
    pub(crate) client_id: i64,
    pub(crate) correlation_id: i64,
}

impl<A> Flyweight<A, CorrelatedMessageDefn>
where
    A: AtomicBuffer,
{
    pub fn client_id(&self) -> i64 {
        self.get_struct().client_id
    }

    pub fn put_client_id(&mut self, value: i64) -> &mut Self {
        self.get_struct_mut().client_id = value;
        self
    }

    pub fn correlation_id(&self) -> i64 {
        self.get_struct().correlation_id
    }

    pub fn put_correlation_id(&mut self, value: i64) -> &mut Self {
        self.get_struct_mut().correlation_id = value;
        self
    }
}
