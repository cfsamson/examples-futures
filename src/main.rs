fn main() {
    let mut data = Data::new(3, 2);
    let vtable = vec![
        0,            // pointer to `Drop`
        6,            // Lenght of vtable
        8,            // alignment
        add as usize, // function
        sub as usize, // function
        mul as usize, // function
    ];

    let fat_pointer = FatPointer::new(&mut data, vtable.as_ptr());

    // we coherce the reference to FatPointer to a pointer to a FatPointer which we can then re-cast
    let fat_pointer_ptr: *const FatPointer = &fat_pointer;
    // we then cast the pointer to our FatPointer as a pointer to a pointer to a Trait object
    // (remember that FatPointer struct is a representation of the pointer to our trait object
    // since it contains two pointers in the form of references to the data + vtable)
    let trait_ptr: *const *const dyn Test = fat_pointer_ptr as *const *const dyn Test;
    // dereference the trait object fully (and take a reference to it instead)
    let test: &dyn Test = unsafe { &**trait_ptr };

    // And voalá it's now a trait object we can call methods on
    println!("Add: {}", test.add());
    println!("Sub: {}", test.sub());
    println!("Mul: {}", test.mul());

    // A simpler way to convert it is this
    // 
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
