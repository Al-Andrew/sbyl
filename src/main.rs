
#[repr(u64)]
enum Opcode {
    Halt,
    SetRegImm,
    SetRegReg,
    AddRegReg,
    AddRegImm,
    CmpRegImm,
    JumpEq,
    JumpNotEq,
    DebugRegs,
}

impl Opcode {
    pub fn from_u64(input: u64) -> Result<Opcode, String> {
        match input {
            0 => { return Ok(Opcode::Halt); }
            1 => { return Ok(Opcode::SetRegImm); }
            2 => { return Ok(Opcode::SetRegReg); }
            3 => { return Ok(Opcode::AddRegReg); }
            4 => { return Ok(Opcode::AddRegImm); }
            5 => { return Ok(Opcode::CmpRegImm); }
            6 => { return Ok(Opcode::JumpEq); }
            7 => { return Ok(Opcode::JumpNotEq); }
            8 => { return Ok(Opcode::DebugRegs); }
            _ => { return Err(format!{"unkown opcode: {}", input}); }
        }

    }
}

#[derive(Debug)]
enum Instruction {
    Halt,
    SetRegImm(u64, u64),
    SetRegReg(u64, u64),
    AddRegReg(u64, u64),
    AddRegImm(u64, u64),
    CmpRegImm(u64, u64),
    JumpNotEq(u64),

    DebugRegs,
}

impl Instruction {
    pub fn execute(self: Self, vm: &mut VM) -> Result<bool, String> {
        // println!("{:?}", self);
        // vm.print_regs();
        match self {
            Self::SetRegImm(dst, value) => {
                vm.registers.reg_arr[dst as usize] = value;
            },
            Self::SetRegReg(dst, src) => {
                vm.registers.reg_arr[dst as usize] = vm.registers.reg_arr[src as usize];
            },
            Self::AddRegReg(t1, t2) => {
                vm.registers.reg_arr[2] = vm.registers.reg_arr[t1 as usize] + vm.registers.reg_arr[t2 as usize];
            },
            Self::AddRegImm(t1, t2) => {
                vm.registers.reg_arr[2] = vm.registers.reg_arr[t1 as usize] + t2;
            },
            Self::CmpRegImm(reg, imm) => {
                let regval = vm.registers.reg_arr[reg as usize];
                vm.registers.reg_arr[2] = if regval > imm { 0 } else if regval == imm { 1 } else { 2 };
            }
            Self::JumpNotEq(addr) => {
                if vm.registers.reg_arr[2] != 1 {
                    vm.registers.reg_arr[0] = addr;
                }
            }
            Self::DebugRegs => {
                vm.print_regs();
            }
            Self::Halt => {
                return Ok(false);
            }
        }
        return Ok(true);
    }
}



struct Registers {
    //0 pc: u64,
    //1 sp: u64,
    //2 ac: u64,
    //3 fl: u64,
    //4 r1: u64,
    //5 r2: u64,
    //6 r3: u64,
    //7 r4: u64,
    reg_arr: [u64; 8],
}

struct VM {
    registers: Registers,
    _stack: [u8; 4096],
}

impl VM {

    pub fn print_regs(self: &Self) {
        println!("registers: ");
        println!("    pc: {}", self.registers.reg_arr[0]);
        println!("    sp: {}", self.registers.reg_arr[1]);
        println!("    ac: {}", self.registers.reg_arr[2]);
        println!("    fl: {}", self.registers.reg_arr[3]);
        println!("    r1: {}", self.registers.reg_arr[4]);
        println!("    r2: {}", self.registers.reg_arr[5]);
        println!("    r3: {}", self.registers.reg_arr[6]);
        println!("    r4: {}", self.registers.reg_arr[7]);
    }

    pub fn execute(self: &mut Self, bytecode: Vec<u64>) -> Result<(), String> {
        
        
        loop {
            let byte: u64 = bytecode[self.registers.reg_arr[0] as usize];
            let instr :Instruction = match Opcode::from_u64(byte)? {
                Opcode::SetRegImm => {
                    let reg = bytecode[(self.registers.reg_arr[0] + 1) as usize];
                    let value = bytecode[(self.registers.reg_arr[0] + 2) as usize];
                    self.registers.reg_arr[0] += 3;

                    Instruction::SetRegImm(reg, value)
                },
                Opcode::SetRegReg => {
                    let dst = bytecode[(self.registers.reg_arr[0] + 1) as usize];
                    let src = bytecode[(self.registers.reg_arr[0] + 2) as usize];
                    self.registers.reg_arr[0] += 3;
                    
                    Instruction::SetRegReg(dst, src)
                },
                Opcode::AddRegImm => {
                    let t1 = bytecode[(self.registers.reg_arr[0] + 1) as usize];
                    let t2 = bytecode[(self.registers.reg_arr[0] + 2) as usize];
                    self.registers.reg_arr[0] += 3;
                    
                    Instruction::AddRegImm(t1, t2)
                },
                Opcode::AddRegReg => {
                    let t1 = bytecode[(self.registers.reg_arr[0] + 1) as usize];
                    let t2 = bytecode[(self.registers.reg_arr[0] + 2) as usize];
                    self.registers.reg_arr[0] += 3;
                    
                    Instruction::AddRegReg(t1, t2)
                },
                Opcode::CmpRegImm => {
                    let reg = bytecode[(self.registers.reg_arr[0] + 1) as usize];
                    let imm = bytecode[(self.registers.reg_arr[0] + 2) as usize];
                    self.registers.reg_arr[0] += 3;
                    
                    Instruction::CmpRegImm(reg, imm)
                },
                Opcode::JumpNotEq => {
                    let addr = bytecode[(self.registers.reg_arr[0] + 1) as usize];
                    self.registers.reg_arr[0] += 2;
                    
                    Instruction::JumpNotEq(addr)
                },
                Opcode::DebugRegs => { self.registers.reg_arr[0] += 1; Instruction::DebugRegs },
                Opcode::Halt => { self.registers.reg_arr[0] += 1; Instruction::Halt },
                _ => todo!()
            };
            if !instr.execute(self)? {
                break;
            }
        }

        return Ok(());
    }
}


fn main() -> Result<(), String> {
    // let source = std::fs::read_to_string("tests/simplest.sbyl")?;

    let bytecode :Vec<u64> = vec![
        Opcode::SetRegImm as u64, 4, 0, // set $r1 0
        Opcode::SetRegImm as u64, 5, 1, // set $r2 1
        Opcode::SetRegImm as u64, 6, 0, // set $r3 0
        // loop:
        Opcode::AddRegReg as u64, 4, 5, // $ac = $r1 + $r2
        Opcode::SetRegReg as u64, 4, 5, // $r1 = $r2
        Opcode::SetRegReg as u64, 5, 2, // $r2 = $ac

        Opcode::AddRegImm as u64, 6, 1, // $ac = $r3 + 1
        Opcode::SetRegReg as u64, 6, 2, // $r3 = $ac
        Opcode::CmpRegImm as u64, 6, 55, // if $r3 < 55
        Opcode::JumpNotEq as u64, 9, // goto loop

        Opcode::DebugRegs as u64, 
        Opcode::Halt as u64,
    ];
    let mut vm: VM = VM{registers: Registers{reg_arr: [0; 8]}, _stack: [0; 4096]};
    vm.execute(bytecode)?;

    return Ok(());
}
