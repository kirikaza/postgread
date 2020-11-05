use ::std::marker::PhantomData;
use ::std::sync::Mutex;

/// Sets something up before all tests and and tears that down after all tests.
pub trait GlobalFixture {
    fn setup() -> Result<(Self::TestContext, Self::TearDownHandle), String>;
    fn tear_down(handle: Self::TearDownHandle) -> Result<(), String>;
    type TestContext: 'static;
    type TearDownHandle: 'static;
    fn get_mutex_context() -> &'static GlobalMutexContext<Self::TestContext, Self::TearDownHandle>;
    const TESTS_FILE_CONTENT: &'static str;
}

pub struct GlobalContext<TestContext, TearDownHandle> {
    tests_total: usize,
    tests_ended: usize,
    test_context: TestContext,
    tear_down_handle: Option<TearDownHandle>,
}

/// Useful alias
pub type GlobalMutexContext<TestContext, TearDownHandle> = Mutex<Option<GlobalContext<TestContext, TearDownHandle>>>;

pub fn new_global_mutex_context<TestContext, TearDownHandle>() -> GlobalMutexContext<TestContext, TearDownHandle> {
    Mutex::new(None)
}

/// Per-test fixture to implement single global fixture.
/// The first created instance:
///   * locks concurrent creations,
///   * counts tests and stores them into global variable,
///   * performs a custom setup action,
/// All subsequent creations do nothing.
/// Each destruction decrements tests count; if no tests left, it performs a custom teardown action.
pub struct BoundFixture<GF>
where
    GF: GlobalFixture,
    GF::TestContext: Clone,
{
    pub test_context: GF::TestContext,
    _global_fixture: PhantomData<GF>,
}

impl<GF> BoundFixture<GF>
where
    GF: GlobalFixture,
    GF::TestContext: Clone,
{
    pub fn new() -> Self {
        let mut context_guard = GF::get_mutex_context().lock().expect("could not lock the mutex for the global fixture");
        if let Some(global_context) = &*context_guard {
            Self {
                test_context: global_context.test_context.clone(),
                _global_fixture: PhantomData,
            }
        } else {
            let tests_count = count_tests(GF::TESTS_FILE_CONTENT);
            let (test_context, tear_down_handle) = GF::setup().expect("could not setup the global fixture");
            let this = Self {
                test_context: test_context.clone(),
                _global_fixture: PhantomData,
            };
            let global_context = GlobalContext {
                tests_total: tests_count,
                tests_ended: 0,
                test_context,
                tear_down_handle: Some(tear_down_handle),
            };
            *context_guard = Some(global_context);
            this
        }
    }
}

impl<GF> Drop for BoundFixture<GF>
where
    GF: GlobalFixture,
    GF::TestContext: Clone,
{
    fn drop(&mut self) {
        let mut context_guard = GF::get_mutex_context().lock().expect("could not lock the mutex for the global fixture");
        let context: &mut GlobalContext<_, _> = context_guard.as_mut().expect("global context should be set before all tests");
        let tests_ended = &mut context.tests_ended;
        let tests_total = context.tests_total;
        if *tests_ended >= tests_total {
            panic!("found an extra test ({} of {}) after the global fixture torn down", *tests_ended + 1, tests_total)
        } else if *tests_ended + 1 == tests_total {
            let tear_down_handle = context.tear_down_handle.take().expect("could not take tear down handle; probably, it is an internal bug");
            GF::tear_down(tear_down_handle).expect("couldn't tear down the global fixture");
        }
        *tests_ended += 1;
    }
}

fn count_tests(tests_file_content: &'static str) -> usize {
    tests_file_content
        .lines()
        .filter(|ln| ln.starts_with("#[rstest]"))
        .count()
}
