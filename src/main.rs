use reglang::{Operand, Vm, fib_program};

fn main() {
    let mut vm = Vm::new(fib_program());
    vm.run();

    println!("Final registers:");
    println!("r0 = {}", vm.read_operand(Operand::Register(0)));
    println!("r1 = {}", vm.read_operand(Operand::Register(1)));
    println!("r2 = {}", vm.read_operand(Operand::Register(2)));
    println!("r3 = {}", vm.read_operand(Operand::Register(3)));
}
