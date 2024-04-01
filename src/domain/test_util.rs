use anyhow::anyhow;

/// Connectivity represents the "connected" state of a mocked driven port and provides
/// common behavior for returning an error if the port is configured to be in a disconnected state.
pub enum Connectivity {
    Connected,
    Disconnected,
}

impl Connectivity {
    /// Return an error if connectivity is in a "disconnected" state
    pub fn blow_up_if_disconnected(&self) -> Result<(), anyhow::Error> {
        match self {
            Self::Connected => Ok(()),
            Self::Disconnected => Err(anyhow!("could not connect to service!")),
        }
    }
}

/// FakeImplementation is a quick drop-in property that helps mock a function and capture
/// arguments the function is called with. It's useful for mocking async functions since
/// popular rust mocking tools don't work well with async functions on traits.
///
/// * [Args] represents the arguments passed to the function that should be captured on a call
/// * [Ret] represents the type of the function's return value
///
/// # Example
///
/// This data structure can be used in mock trait implementations like so:
///
/// ```
/// use domain::test_util::FakeImplementation;
/// use std::sync::Mutex;
///
/// trait MyAsyncTrait {
///   async fn some_cool_function(&self, var_1: i32, var_2: i32) -> String;
/// }
///
/// struct FakeTraitImplementation {
///   // The generics are (i32, i32) for captured arguments and String for the return value
///   some_cool_function_result: FakeImplementation<(i32, i32), String>;
/// }
///
/// impl MyAsyncTrait for Mutex<FakeTraitImplementation> {
///   async fn some_cool_function(&self, var_1: i32, var_2: i32) -> String {
///     // We have to lock "self" so we can mutate the interior via an immutable reference
///     let mut self_locked = self.lock().unwrap();
///     
///     // Capture the arguments of this invocation
///     self_locked.save_arguments((var_1, var_2));
///
///     // Return the configured return value
///     self_locked.return_value()
///   }
/// }
/// ```
///
pub struct FakeImplementation<Args, Ret> {
    saved_arguments: Vec<Args>,
    return_value: Option<Ret>,
}

impl<Args, Ret> FakeImplementation<Args, Ret> {
    /// Creates a new FakeImplementation
    pub fn new() -> FakeImplementation<Args, Ret> {
        FakeImplementation {
            saved_arguments: Vec::new(),
            return_value: None,
        }
    }
}

impl<Args, Ret> FakeImplementation<Args, Ret> {
    /// Saves arguments from a single invocation of the FakeImplementation
    pub fn save_arguments(&mut self, arguments: Args) {
        self.saved_arguments.push(arguments)
    }

    /// Returns the list of arguments passed on every call to this FakeImplementation
    pub fn calls(&self) -> &[Args] {
        self.saved_arguments.as_slice()
    }
}

#[allow(dead_code)]
impl<Args, Ret> FakeImplementation<Args, Ret>
where
    Ret: Clone,
{
    /// Set the value that should be returned when this FakeImplementation is invoked
    pub fn set_return_value(&mut self, return_value: Ret) {
        self.return_value = Some(return_value)
    }

    /// Retrieve the configured return value for this FakeImplementation
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
    /// Set the result that should be returned when this FakeImplementation is invoked.
    /// [Result] does not implement [Clone], so this function can be used when the contained values
    /// can be cloned.
    pub fn set_returned_result(&mut self, return_value: Result<Success, Fail>) {
        match return_value {
            Ok(ok_result) => self.return_value = Some(Ok(ok_result)),
            Err(err) => self.return_value = Some(Err(err)),
        }
    }

    /// Retrieve the result that should be returned when this FakeImplementation is invoked (for [Result]s)
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
    /// Set the result that should be returned when this FakeImplementation is invoked.
    /// This is used in a special case for [anyhow::Result], since [anyhow::Error] does not
    /// implement [Clone].
    pub fn set_returned_anyhow(&mut self, return_value: anyhow::Result<Success>) {
        match return_value {
            Ok(ok_result) => self.return_value = Some(Ok(ok_result)),
            Err(err) => self.return_value = Some(Err(anyhow!(format!("{}", err)))),
        }
    }

    /// Retrieve the result that should be returned when this FakeImplementation is invoked (for [anyhow::Result]s)
    pub fn return_value_anyhow(&self) -> anyhow::Result<Success> {
        match self.return_value {
            None => panic!("Tried to return from a function where the value wasn't set!"),
            Some(Ok(ref ok_result)) => Ok(ok_result.clone()),
            Some(Err(ref err)) => Err(anyhow!(format!("{}", err))),
        }
    }
}
