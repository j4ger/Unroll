#[cfg(test)]
#[macro_use]
extern crate lazy_static;

mod tests {
    use poca_server::{DataHandle, Poca};

    lazy_static! {
        static ref POCA: Poca = Poca::new("localhost:1120");
        static ref HANDLE1: DataHandle<i32> = POCA.data("test1", 1);
    }

    //? not sure if this always works
    #[test]
    fn on_change_handler_with_inner_self_set() {
        HANDLE1.on_change(|_new_value| {
            HANDLE1.set(2);
        })
    }
}