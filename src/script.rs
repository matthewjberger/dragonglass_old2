use gluon::{import::add_extern_module, new_vm, primitive, vm, RootedThread, Thread, ThreadExt};

fn factorial(x: i32) -> i32 {
    if x <= 1 {
        1
    } else {
        x * factorial(x - 1)
    }
}

fn load_factorial(vm: &Thread) -> vm::Result<vm::ExternModule> {
    vm::ExternModule::new(vm, primitive!(1, factorial))
}

struct ScriptVirtualMachine {
    pub vm: RootedThread,
}

impl ScriptVirtualMachine {
    pub fn new() -> Self {
        Self { vm: new_vm() }
    }

    pub fn test(&self) {
        // Introduce a module that can be loaded with `import! factorial`
        add_extern_module(&self.vm, "factorial", load_factorial);

        let expr = r#"
let factorial = import! factorial
factorial 5
"#;

        let (result, _) = self.vm.run_expr::<i32>("factorial", expr).unwrap();

        assert_eq!(result, 120);
    }
}
