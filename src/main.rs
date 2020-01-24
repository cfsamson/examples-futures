fn main() {
    let mut data = Data::new(2, 3);
    let vtable = vec![
        0,            // pointer to `Drop`
        6,            // Lenght of vtable
        8,            // alignment
        add as usize, // function
        sub as usize, // function
        mul as usize, // function
    ];

    let fat_pointer = FatPointer::new(&mut data, vtable.as_ptr());

    // we coherce the reference to FatPointer as a pointer
    let fat_pointer_ptr: *const FatPointer = &fat_pointer;
    // we then cast the pointer to our FatPointer as a pointer to a pointer to a Trait object
    // remember that FatPointer is the pointer to our trait object
    let trait_ptr: *const *const dyn Test = fat_pointer_ptr as *const *const dyn Test;
    // dereference the traot object (and take a reference to it instead)
    let test: &dyn Test = unsafe { &**trait_ptr };
    println!("Add: {}", test.add());
    println!("Sub: {}", test.sub());
    println!("Mul: {}", test.mul());

    // Or this can be simpler
    // let test = unsafe { std::mem::transmute::<FatPointer, &dyn Test>(fat_pointer) };
    // println!("ADD: {}", test.add());
}

trait Test {
    fn add(&self) -> i32;
    fn sub(&self) -> i32;
    fn mul(&self) -> i32;
}

#[repr(C)]
struct FatPointer<'a> {
    data: &'a mut Data,
    vtable: *const usize,
}

impl<'a> FatPointer<'a> {
    fn new(data: &'a mut Data, vtable: *const usize) -> Self {
        FatPointer { data, vtable }
    }
}

struct Data {
    a: i32,
    b: i32,
}

impl Data {
    fn new(a: i32, b: i32) -> Self {
        Data { a, b }
    }
}

fn add(s: &Data) -> i32 {
    s.a + s.b
}
fn sub(s: &Data) -> i32 {
    s.a - s.b
}
fn mul(s: &Data) -> i32 {
    s.a * s.b
}
