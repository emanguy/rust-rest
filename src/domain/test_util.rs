use anyhow::anyhow;

pub enum Connectivity {
    Connected,
    Disconnected,
}

impl Connectivity {
    pub fn blow_up_if_disconnected(&self) -> Result<(), anyhow::Error> {
        match self {
            Self::Connected => Ok(()),
            Self::Disconnected => Err(anyhow!("could not connect to service!")),
        }
    }
}

pub struct FakeImplementation<Args, Ret> {
    saved_arguments: Vec<Args>,
    return_value: Option<Ret>,
}

impl<Args, Ret> FakeImplementation<Args, Ret> {
    pub fn new() -> FakeImplementation<Args, Ret> {
        FakeImplementation {
            saved_arguments: Vec::new(),
            return_value: None,
        }
    }
}

impl<Args, Ret> FakeImplementation<Args, Ret> {
    pub fn save_arguments(&mut self, arguments: Args) {
        self.saved_arguments.push(arguments)
    }

    pub fn calls(&self) -> &[Args] {
        self.saved_arguments.as_slice()
    }
}

impl<Args, Ret> FakeImplementation<Args, Ret>
where
    Ret: Clone,
{
    pub fn set_return_value(&mut self, return_value: Ret) {
        self.return_value = Some(return_value)
    }

    pub fn return_value(&self) -> Ret {
        match self.return_value {
            None => panic!("Tried to return from a function where the return value wasn't set!"),
            Some(ref ret_val) => ret_val.clone(),
        }
    }
}

impl<Args, Success, Fail> FakeImplementation<Args, Result<Success, Fail>>
where
    Success: Clone,
    Fail: Clone,
{
    pub fn set_returned_result(&mut self, return_value: Result<Success, Fail>) {
        match return_value {
            Ok(ok_result) => self.return_value = Some(Ok(ok_result)),
            Err(err) => self.return_value = Some(Err(err)),
        }
    }

    pub fn return_value_result(&self) -> Result<Success, Fail> {
        match self.return_value {
            Some(Ok(ref ok_result)) => Ok(ok_result.clone()),
            Some(Err(ref err)) => Err(err.clone()),
            None => panic!("Tried to return from a function where the return value wasn't set!"),
        }
    }
}

impl<Args, Success> FakeImplementation<Args, anyhow::Result<Success>>
where
    Success: Clone,
{
    pub fn set_returned_anyhow(&mut self, return_value: anyhow::Result<Success>) {
        match return_value {
            Ok(ok_result) => self.return_value = Some(Ok(ok_result)),
            Err(err) => self.return_value = Some(Err(anyhow!(format!("{}", err)))),
        }
    }

    pub fn return_value_anyhow(&self) -> anyhow::Result<Success> {
        match self.return_value {
            None => panic!("Tried to return from a function where the value wasn't set!"),
            Some(Ok(ref ok_result)) => Ok(ok_result.clone()),
            Some(Err(ref err)) => Err(anyhow!(format!("{}", err))),
        }
    }
}
