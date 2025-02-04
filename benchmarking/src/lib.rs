//! Macro for benchmarking a Substrate runtime. A fork of `frame-benchmarking`
//! pallet.

#![cfg_attr(not(feature = "std"), no_std)]

mod tests;

pub use frame_benchmarking::{
	benchmarking, whitelisted_caller, BenchmarkBatch, BenchmarkConfig, BenchmarkParameter, BenchmarkResults,
	Benchmarking, BenchmarkingSetup, TrackedStorageKey,
};
#[cfg(feature = "std")]
pub use frame_benchmarking::{Analysis, BenchmarkSelector};
pub use frame_support;
pub use paste;
#[doc(hidden)]
pub use sp_io::storage::root as storage_root;
pub use sp_runtime::traits::Zero;

/// Construct pallet benchmarks for weighing dispatchables.
///
/// Works around the idea of complexity parameters, named by a single letter
/// (which is usually upper cased in complexity notation but is lower-cased for
/// use in this macro).
///
/// Complexity parameters ("parameters") have a range which is a `u32` pair.
/// Every time a benchmark is prepared and run, this parameter takes a concrete
/// value within the range. There is an associated instancing block, which is a
/// single expression that is evaluated during preparation. It may use `?`
/// (`i.e. `return Err(...)`) to bail with a string error. Here's a
/// few examples:
///
/// ```ignore
/// // These two are equivalent:
/// let x in 0 .. 10;
/// let x in 0 .. 10 => ();
/// // This one calls a setup function and might return an error (which would be terminal).
/// let y in 0 .. 10 => setup(y)?;
/// // This one uses a code block to do lots of stuff:
/// let z in 0 .. 10 => {
///   let a = z * z / 5;
///   let b = do_something(a)?;
///   combine_into(z, b);
/// }
/// ```
///
/// Note that due to parsing restrictions, if the `from` expression is not a
/// single token (i.e. a literal or constant), then it must be parenthesised.
///
/// The macro allows for a number of "arms", each representing an individual
/// benchmark. Using the simple syntax, the associated dispatchable function
/// maps 1:1 with the benchmark and the name of the benchmark is the same as
/// that of the associated function. However, extended syntax allows
/// for arbitrary expresions to be evaluated in a benchmark (including for
/// example, `on_initialize`).
///
/// The macro allows for common parameters whose ranges and instancing
/// expressions may be drawn upon (or not) by each arm. Syntax is available to
/// allow for only the range to be drawn upon if desired, allowing an
/// alternative instancing expression to be given.
///
/// Note that the ranges are *inclusive* on both sides. This is in contrast to
/// ranges in Rust which are left-inclusive right-exclusive.
///
/// Each arm may also have a block of code which is run prior to any instancing
/// and a block of code which is run afterwards. All code blocks may draw upon
/// the specific value of each parameter at any time. Local variables are shared
/// between the two pre- and post- code blocks, but do not leak from the
/// interior of any instancing expressions.
///
/// Any common parameters that are unused in an arm do not have their instancing
/// expressions evaluated.
///
/// Example:
/// ```ignore
/// use path_to_node_runtime::MyRuntime;
/// use path_to_account_id::AccountId;
/// use frame_benchmarking::account;
/// use orml_benchmarking::runtime_benchmarks;
///
/// runtime_benchmarks! {
///   // The constructed runtime struct, and the pallet to benchmark.
///   { MyRuntime, my_pallet }
///
///
///   // first dispatchable: foo; this is a user dispatchable and operates on a `u8` vector of
///   // size `l`, which we allow to be initialized as usual.
///   foo {
///     let caller = account::<AccountId>(b"caller", 0, benchmarks_seed);
///     let l = ...;
///   }: _(Origin::Signed(caller), vec![0u8; l])
///
///   // second dispatchable: bar; this is a root dispatchable and accepts a `u8` vector of size
///   // `l`. We don't want it pre-initialized like before so we override using the `=> ()` notation.
///   // In this case, we explicitly name the call using `bar` instead of `_`.
///   bar {
///     let l = _ .. _ => ();
///   }: bar(Origin::Root, vec![0u8; l])
///
///   // third dispatchable: baz; this is a user dispatchable. It isn't dependent on length like the
///   // other two but has its own complexity `c` that needs setting up. It uses `caller` (in the
///   // pre-instancing block) within the code block. This is only allowed in the param instancers
///   // of arms. Instancers of common params cannot optimistically draw upon hypothetical variables
///   // that the arm's pre-instancing code block might have declared.
///   baz1 {
///     let caller = account::<AccountId>(b"caller", 0, benchmarks_seed);
///     let c = 0 .. 10 => setup_c(&caller, c);
///   }: baz(Origin::Signed(caller))
///
///   // this is a second benchmark of the baz dispatchable with a different setup.
///   baz2 {
///     let caller = account::<AccountId>(b"caller", 0, benchmarks_seed);
///     let c = 0 .. 10 => setup_c_in_some_other_way(&caller, c);
///   }: baz(Origin::Signed(caller))
///
///   // this is benchmarking some code that is not a dispatchable.
///   populate_a_set {
///     let x in 0 .. 10_000;
///     let mut m = Vec::<u32>::new();
///     for i in 0..x {
///       m.insert(i);
///     }
///   }: { m.into_iter().collect::<BTreeSet>() }
/// }
/// ```
///
/// Test functions are automatically generated for each benchmark and are
/// accessible to you when you run `cargo test`. All tests are named
/// `test_benchmark_<benchmark_name>`, expect you to pass them the Runtime
/// Config, and run them in a test externalities environment. The test function
/// runs your benchmark just like a regular benchmark, but only testing at the
/// lowest and highest values for each component. The function will return
/// `Ok(())` if the benchmarks return no errors.
///
/// You can optionally add a `verify` code block at the end of a benchmark to
/// test any final state of your benchmark in a unit test. For example:
///
/// ```ignore
/// sort_vector {
///     let x in 1 .. 10000;
///     let mut m = Vec::<u32>::new();
///     for i in (0..x).rev() {
///         m.push(i);
///     }
/// }: {
///     m.sort();
/// } verify {
///     ensure!(m[0] == 0, "You forgot to sort!")
/// }
/// ```
///
/// These `verify` blocks will not affect your benchmark results!
///
/// You can construct benchmark tests like so:
///
/// ```ignore
/// #[test]
/// fn test_benchmarks() {
///   new_test_ext().execute_with(|| {
///     assert_ok!(test_benchmark_dummy());
///     assert_err!(test_benchmark_other_name(), "Bad origin");
///     assert_ok!(test_benchmark_sort_vector());
///     assert_err!(test_benchmark_broken_benchmark(), "You forgot to sort!");
///   });
/// }
/// ```
#[macro_export]
macro_rules! runtime_benchmarks {
	(
		{ $runtime:ident, $pallet:ident }
		_ {
			$(
				let $common:ident in $common_from:tt .. $common_to:expr => $common_instancer:expr;
			)*
		}
		$( $rest:tt )*
	) => {
		$crate::benchmarks_iter!(
			{ }
			$runtime
			$pallet
			{ $( { $common , $common_from , $common_to , $common_instancer } )* }
			( )
			( )
			$( $rest )*
		);
	}
}

/// Same as [`benchmarks`] but for instantiable module.
#[macro_export]
macro_rules! runtime_benchmarks_instance {
	(
		{ $runtime:ident, $pallet:ident, $instance:ident }
		_ {
			$(
				let $common:ident in $common_from:tt .. $common_to:expr => $common_instancer:expr;
			)*
		}
		$( $rest:tt )*
	) => {
		$crate::benchmarks_iter!(
			{ $instance }
			$runtime
			$pallet
			{ $( { $common , $common_from , $common_to , $common_instancer } )* }
			( )
			( )
			$( $rest )*
		);
	}
}

#[macro_export]
#[doc(hidden)]
macro_rules! benchmarks_iter {
	// detect and extract extra tag:
	(
		{ $( $instance:ident )? }
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		( $( $names:tt )* )
		( $( $names_extra:tt )* )
		#[extra]
		$name:ident
		$( $rest:tt )*
	) => {
		$crate::benchmarks_iter! {
			{ $( $instance)? }
			$runtime
			$pallet
			{ $( $common )* }
			( $( $names )* )
			( $( $names_extra )* $name )
			$name
			$( $rest )*
		}
	};
	// mutation arm:
	(
		{ $( $instance:ident )? }
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		( $( $names:tt )* ) // This contains $( $( { $instance } )? $name:ident )*
		( $( $names_extra:tt )* )
		$name:ident { $( $code:tt )* }: _ ( $origin:expr $( , $arg:expr )* )
		verify $postcode:block
		$( $rest:tt )*
	) => {
		$crate::benchmarks_iter! {
			{ $( $instance)? }
			$runtime
			$pallet
			{ $( $common )* }
			( $( $names )* )
			( $( $names_extra )* )
			$name { $( $code )* }: $name ( $origin $( , $arg )* )
			verify $postcode
			$( $rest )*
		}
	};
	// mutation arm:
	(
		{ $( $instance:ident )? }
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		( $( $names:tt )* )
		( $( $names_extra:tt )* )
		$name:ident { $( $code:tt )* }: $dispatch:ident ( $origin:expr $( , $arg:expr )* )
		verify $postcode:block
		$( $rest:tt )*
	) => {
		$crate::benchmarks_iter! {
			{ $( $instance)? }
			$runtime
			$pallet
			{ $( $common )* }
			( $( $names )* )
			( $( $names_extra )* )
			$name { $( $code )* }: {
				<
					$pallet::Call<$runtime $(, $instance)? > as $crate::frame_support::traits::UnfilteredDispatchable
				>
				::dispatch_bypass_filter(
					$pallet::Call::<$runtime $(, $instance)? >::$dispatch($($arg),*), $origin.into()
				)?;
			}
			verify $postcode
			$( $rest )*
		}
	};
	// iteration arm:
	(
		{ $( $instance:ident )? }
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		( $( $names:tt )* )
		( $( $names_extra:tt )* )
		$name:ident { $( $code:tt )* }: $eval:block
		verify $postcode:block
		$( $rest:tt )*
	) => {
		$crate::benchmark_backend! {
			{ $( $instance)? }
			$name
			$runtime
			$pallet
			{ $( $common )* }
			{ }
			{ $eval }
			{ $( $code )* }
			$postcode
		}

		#[cfg(test)]
		$crate::impl_benchmark_test!(
			$runtime
			$pallet
			{ $( $instance)? }
			$name
		);

		$crate::benchmarks_iter!(
			{ $( $instance)? }
			$runtime
			$pallet
			{ $( $common )* }
			( $( $names )* { $( $instance )? } $name )
			( $( $names_extra )* )
			$( $rest )*
		);
	};
	// iteration-exit arm
	(
		{ $( $instance:ident )? }
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		( $( $names:tt )* )
		( $( $names_extra:tt )* )
	) => {
		$crate::selected_benchmark!(
			$runtime
			$pallet
			{ $( $instance)? }
			$( $names )*
		);
		$crate::impl_benchmark!(
			$runtime
			$pallet
			{ $( $instance)? }
			( $( $names )* )
			( $( $names_extra ),* )
		);
	};
	// add verify block to _() format
	(
		{ $( $instance:ident )? }
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		( $( $names:tt )* )
		( $( $names_extra:tt )* )
		$name:ident { $( $code:tt )* }: _ ( $origin:expr $( , $arg:expr )* )
		$( $rest:tt )*
	) => {
		$crate::benchmarks_iter! {
			{ $( $instance)? }
			$runtime
			$pallet
			{ $( $common )* }
			( $( $names )* )
			( $( $names_extra )* )
			$name { $( $code )* }: _ ( $origin $( , $arg )* )
			verify { }
			$( $rest )*
		}
	};
	// add verify block to name() format
	(
		{ $( $instance:ident )? }
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		( $( $names:tt )* )
		( $( $names_extra:tt )* )
		$name:ident { $( $code:tt )* }: $dispatch:ident ( $origin:expr $( , $arg:expr )* )
		$( $rest:tt )*
	) => {
		$crate::benchmarks_iter! {
			{ $( $instance)? }
			$runtime
			$pallet
			{ $( $common )* }
			( $( $names )* )
			( $( $names_extra )* )
			$name { $( $code )* }: $dispatch ( $origin $( , $arg )* )
			verify { }
			$( $rest )*
		}
	};
	// add verify block to {} format
	(
		{ $( $instance:ident )? }
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		( $( $names:tt )* )
		( $( $names_extra:tt )* )
		$name:ident { $( $code:tt )* }: $eval:block
		$( $rest:tt )*
	) => {
		$crate::benchmarks_iter!(
			{ $( $instance)? }
			$runtime
			$pallet
			{ $( $common )* }
			( $( $names )* )
			( $( $names_extra )* )
			$name { $( $code )* }: $eval
			verify { }
			$( $rest )*
		);
	};
}

#[macro_export]
#[doc(hidden)]
macro_rules! benchmark_backend {
	// parsing arms
	(
		{ $( $instance:ident )? }
		$name:ident
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		{ $( PRE { $( $pre_parsed:tt )* } )* }
		{ $eval:block }
		{
			let $pre_id:tt : $pre_ty:ty = $pre_ex:expr;
			$( $rest:tt )*
		}
		$postcode:block
	) => {
		$crate::benchmark_backend! {
			{ $( $instance)? }
			$name
			$runtime
			$pallet
			{ $( $common )* }
			{
				$( PRE { $( $pre_parsed )* } )*
				PRE { $pre_id , $pre_ty , $pre_ex }
			}
			{ $eval }
			{ $( $rest )* }
			$postcode
		}
	};
	(
		{ $( $instance:ident )? }
		$name:ident
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		{ $( $parsed:tt )* }
		{ $eval:block }
		{
			let $param:ident in ( $param_from:expr ) .. $param_to:expr => $param_instancer:expr;
			$( $rest:tt )*
		}
		$postcode:block
	) => {
		$crate::benchmark_backend! {
			{ $( $instance)? }
			$name
			$runtime
			$pallet
			{ $( $common )* }
			{
				$( $parsed )*
				PARAM { $param , $param_from , $param_to , $param_instancer }
			}
			{ $eval }
			{ $( $rest )* }
			$postcode
		}
	};
	// mutation arm to look after defaulting to a common param
	(
		{ $( $instance:ident )? }
		$name:ident
		$runtime:ident
		$pallet:ident
		{ $( { $common:ident , $common_from:tt , $common_to:expr , $common_instancer:expr } )* }
		{ $( $parsed:tt )* }
		{ $eval:block }
		{
			let $param:ident in ...;
			$( $rest:tt )*
		}
		$postcode:block
	) => {
		$crate::benchmark_backend! {
			{ $( $instance)? }
			$name
			$runtime
			$pallet
			{ $( { $common , $common_from , $common_to , $common_instancer } )* }
			{ $( $parsed )* }
			{ $eval }
			{
				let $param
					in ({ $( let $common = $common_from; )* $param })
					.. ({ $( let $common = $common_to; )* $param })
					=> ({ $( let $common = || -> Result<(), &'static str> { $common_instancer ; Ok(()) }; )* $param()? });
				$( $rest )*
			}
			$postcode
		}
	};
	// mutation arm to look after defaulting only the range to common param
	(
		{ $( $instance:ident )? }
		$name:ident
		$runtime:ident
		$pallet:ident
		{ $( { $common:ident , $common_from:tt , $common_to:expr , $common_instancer:expr } )* }
		{ $( $parsed:tt )* }
		{ $eval:block }
		{
			let $param:ident in _ .. _ => $param_instancer:expr ;
			$( $rest:tt )*
		}
		$postcode:block
	) => {
		$crate::benchmark_backend! {
			{ $( $instance)? }
			$name
			$runtime
			$pallet
			{ $( { $common , $common_from , $common_to , $common_instancer } )* }
			{ $( $parsed )* }
			{ $eval }
			{
				let $param
					in ({ $( let $common = $common_from; )* $param })
					.. ({ $( let $common = $common_to; )* $param })
					=> $param_instancer ;
				$( $rest )*
			}
			$postcode
		}
	};
	// mutation arm to look after a single tt for param_from.
	(
		{ $( $instance:ident )? }
		$name:ident
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		{ $( $parsed:tt )* }
		{ $eval:block }
		{
			let $param:ident in $param_from:tt .. $param_to:expr => $param_instancer:expr ;
			$( $rest:tt )*
		}
		$postcode:block
	) => {
		$crate::benchmark_backend! {
			{ $( $instance)? }
			$name
			$runtime
			$pallet
			{ $( $common )* }
			{ $( $parsed )* }
			{ $eval }
			{
				let $param in ( $param_from ) .. $param_to => $param_instancer;
				$( $rest )*
			}
			$postcode
		}
	};
	// mutation arm to look after the default tail of `=> ()`
	(
		{ $( $instance:ident )? }
		$name:ident
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		{ $( $parsed:tt )* }
		{ $eval:block }
		{
			let $param:ident in $param_from:tt .. $param_to:expr;
			$( $rest:tt )*
		}
		$postcode:block
	) => {
		$crate::benchmark_backend! {
			{ $( $instance)? }
			$name
			$runtime
			$pallet
			{ $( $common )* }
			{ $( $parsed )* }
			{ $eval }
			{
				let $param in $param_from .. $param_to => ();
				$( $rest )*
			}
			$postcode
		}
	};
	// mutation arm to look after `let _ =`
	(
		{ $( $instance:ident )? }
		$name:ident
		$runtime:ident
		$pallet:ident
		{ $( $common:tt )* }
		{ $( $parsed:tt )* }
		{ $eval:block }
		{
			let $pre_id:tt = $pre_ex:expr;
			$( $rest:tt )*
		}
		$postcode:block
	) => {
		$crate::benchmark_backend! {
			{ $( $instance)? }
			$name
			$runtime
			$pallet
			{ $( $common )* }
			{ $( $parsed )* }
			{ $eval }
			{
				let $pre_id : _ = $pre_ex;
				$( $rest )*
			}
			$postcode
		}
	};
	// actioning arm
	(
		{ $( $instance:ident )? }
		$name:ident
		$runtime:ident
		$pallet:ident
		{ $( { $common:ident , $common_from:tt , $common_to:expr , $common_instancer:expr } )* }
		{
			$( PRE { $pre_id:tt , $pre_ty:ty , $pre_ex:expr } )*
			$( PARAM { $param:ident , $param_from:expr , $param_to:expr , $param_instancer:expr } )*
		}
		{ $eval:block }
		{ $( $post:tt )* }
		$postcode:block
	) => {
		#[allow(non_camel_case_types)]
		struct $name;
		#[allow(unused_variables)]
		impl $crate::BenchmarkingSetup<$runtime $(, $instance)?> for $name {
			fn components(&self) -> Vec<($crate::BenchmarkParameter, u32, u32)> {
				vec! [
					$(
						($crate::BenchmarkParameter::$param, $param_from, $param_to)
					),*
				]
			}

			fn instance(
				&self,
				components: &[($crate::BenchmarkParameter, u32)],
				verify: bool
			) -> Result<Box<dyn FnOnce() -> Result<(), &'static str>>, &'static str> {
				$(
					let $common = $common_from;
				)*
				$(
					// Prepare instance
					let $param = components.iter()
						.find(|&c| c.0 == $crate::BenchmarkParameter::$param)
						.ok_or("Could not find component in benchmark preparation.")?
						.1;
				)*
				$(
					let $pre_id : $pre_ty = $pre_ex;
				)*
				$( $param_instancer ; )*
				$( $post )*

				Ok(Box::new(move || -> Result<(), &'static str> {
					$eval;
					if verify {
						$postcode;
					}
					Ok(())
				}))
			}
		}
	};
}

// Creates a `SelectedBenchmark` enum implementing `BenchmarkingSetup`.
//
// Every variant must implement [`BenchmarkingSetup`].
//
// ```nocompile
// 
// struct Transfer;
// impl BenchmarkingSetup for Transfer { ... }
//
// struct SetBalance;
// impl BenchmarkingSetup for SetBalance { ... }
//
// selected_benchmark!({} Transfer {} SetBalance);
// ```
#[macro_export]
#[doc(hidden)]
macro_rules! selected_benchmark {
	(
		$runtime:ident
		$pallet:ident
		{ $( $instance:ident )? }
		$( { $( $bench_inst:ident )? } $bench:ident )*
	) => {
		// The list of available benchmarks for this pallet.
		#[allow(non_camel_case_types)]
		enum SelectedBenchmark {
			$( $bench, )*
		}

		// Allow us to select a benchmark from the list of available benchmarks.
		impl $crate::BenchmarkingSetup<$runtime $(, $instance)?> for SelectedBenchmark {
			fn components(&self) -> Vec<($crate::BenchmarkParameter, u32, u32)> {
				match self {
					$(
						Self::$bench => <
							$bench as $crate::BenchmarkingSetup<$runtime $(, $bench_inst)? >
						>::components(&$bench),
					)*
				}
			}

			fn instance(
				&self,
				components: &[($crate::BenchmarkParameter, u32)],
				verify: bool
			)
				-> Result<Box<dyn FnOnce() -> Result<(), &'static str>>, &'static str>
			{
				match self {
					$(
						Self::$bench => <
							$bench as $crate::BenchmarkingSetup<$runtime $(, $bench_inst)? >
						>::instance(&$bench, components, verify),
					)*
				}
			}
		}
	};
}

#[macro_export]
#[doc(hidden)]
macro_rules! impl_benchmark {
	(
		$runtime:ident
		$pallet:ident
		{ $( $instance:ident )? }
		( $( { $( $name_inst:ident )? } $name:ident )* )
		( $( $name_extra:ident ),* )
	) => {
		pub struct Benchmark;

		impl $crate::Benchmarking<$crate::BenchmarkResults> for Benchmark {
			fn benchmarks(extra: bool) -> Vec<&'static [u8]> {
				let mut all = vec![ $( stringify!($name).as_ref() ),* ];
				if !extra {
					let extra = [ $( stringify!($name_extra).as_ref() ),* ];
					all.retain(|x| !extra.contains(x));
				}
				all
			}

			fn run_benchmark(
				extrinsic: &[u8],
				lowest_range_values: &[u32],
				highest_range_values: &[u32],
				steps: &[u32],
				repeat: u32,
				whitelist: &[$crate::TrackedStorageKey],
				verify: bool,
			) -> Result<Vec<$crate::BenchmarkResults>, &'static str> {
				// Map the input to the selected benchmark.
				let extrinsic = sp_std::str::from_utf8(extrinsic)
					.map_err(|_| "`extrinsic` is not a valid utf8 string!")?;
				let selected_benchmark = match extrinsic {
					$( stringify!($name) => SelectedBenchmark::$name, )*
					_ => return Err("Could not find extrinsic."),
				};
				let mut results: Vec<$crate::BenchmarkResults> = Vec::new();
				if repeat == 0 {
					return Ok(results);
				}

				// Add whitelist to DB including whitelisted caller
				let mut whitelist = whitelist.to_vec();
				let whitelisted_caller_key =
					<frame_system::Account::<$runtime> as frame_support::storage::StorageMap<_,_>>::hashed_key_for(
						$crate::whitelisted_caller::<<$runtime as frame_system::Config>::AccountId>()
					);
				whitelist.push(whitelisted_caller_key.into());
				$crate::benchmarking::set_whitelist(whitelist);

				// Warm up the DB
				$crate::benchmarking::commit_db();
				$crate::benchmarking::wipe_db();

				let components = <
					SelectedBenchmark as $crate::BenchmarkingSetup<$runtime $(, $instance)?>
				>::components(&selected_benchmark);

				// Default number of steps for a component.
				let mut prev_steps = 10;

				let repeat_benchmark = |
					repeat: u32,
					c: &[($crate::BenchmarkParameter, u32)],
					results: &mut Vec<$crate::BenchmarkResults>,
					verify: bool,
				| -> Result<(), &'static str> {
					// Run the benchmark `repeat` times.
					for _ in 0..repeat {
						// Set up the externalities environment for the setup we want to
						// benchmark.
						let closure_to_benchmark = <
							SelectedBenchmark as $crate::BenchmarkingSetup<$runtime $(, $instance)?>
						>::instance(&selected_benchmark, c, verify)?;

						// Set the block number to at least 1 so events are deposited.
						if $crate::Zero::is_zero(&frame_system::Module::<$runtime>::block_number()) {
							frame_system::Module::<$runtime>::set_block_number(1u32.into());
						}

						// Commit the externalities to the database, flushing the DB cache.
						// This will enable worst case scenario for reading from the database.
						$crate::benchmarking::commit_db();

						// Reset the read/write counter so we don't count operations in the setup process.
						$crate::benchmarking::reset_read_write_count();

						if verify {
							closure_to_benchmark()?;
						} else {
							// Time the extrinsic logic.
							frame_support::log::trace!(
								target: "benchmark",
								"Start Benchmark: {:?}", c
							);

							let start_extrinsic = $crate::benchmarking::current_time();

							closure_to_benchmark()?;

							let finish_extrinsic = $crate::benchmarking::current_time();
							let elapsed_extrinsic = finish_extrinsic - start_extrinsic;
							// Commit the changes to get proper write count
							$crate::benchmarking::commit_db();
							frame_support::log::trace!(
								target: "benchmark",
								"End Benchmark: {} ns", elapsed_extrinsic
							);
							let read_write_count = $crate::benchmarking::read_write_count();
							frame_support::log::trace!(
								target: "benchmark",
								"Read/Write Count {:?}", read_write_count
							);

							// Time the storage root recalculation.
							let start_storage_root = $crate::benchmarking::current_time();
							$crate::storage_root();
							let finish_storage_root = $crate::benchmarking::current_time();
							let elapsed_storage_root = finish_storage_root - start_storage_root;

							results.push($crate::BenchmarkResults {
								components: c.to_vec(),
								extrinsic_time: elapsed_extrinsic,
								storage_root_time: elapsed_storage_root,
								reads: read_write_count.0,
								repeat_reads: read_write_count.1,
								writes: read_write_count.2,
								repeat_writes: read_write_count.3,
							});
						}

						// Wipe the DB back to the genesis state.
						$crate::benchmarking::wipe_db();
					}

					Ok(())
				};

				if components.is_empty() {
					if verify {
						// If `--verify` is used, run the benchmark once to verify it would complete.
						repeat_benchmark(1, Default::default(), &mut Vec::new(), true)?;
					}
					repeat_benchmark(repeat, Default::default(), &mut results, false)?;
				} else {
					// Select the component we will be benchmarking. Each component will be benchmarked.
					for (idx, (name, low, high)) in components.iter().enumerate() {
						// Get the number of steps for this component.
						let steps = steps.get(idx).cloned().unwrap_or(prev_steps);
						prev_steps = steps;

						// Skip this loop if steps is zero
						if steps == 0 { continue }

						let lowest = lowest_range_values.get(idx).cloned().unwrap_or(*low);
						let highest = highest_range_values.get(idx).cloned().unwrap_or(*high);

						let diff = highest - lowest;

						// Create up to `STEPS` steps for that component between high and low.
						let step_size = (diff / steps).max(1);
						let num_of_steps = diff / step_size + 1;

						for s in 0..num_of_steps {
							// This is the value we will be testing for component `name`
							let component_value = lowest + step_size * s;

							// Select the max value for all the other components.
							let c: Vec<($crate::BenchmarkParameter, u32)> = components.iter()
								.enumerate()
								.map(|(idx, (n, _, h))|
									if n == name {
										(*n, component_value)
									} else {
										(*n, *highest_range_values.get(idx).unwrap_or(h))
									}
								)
								.collect();

							if verify {
								// If `--verify` is used, run the benchmark once to verify it would complete.
								repeat_benchmark(1, &c, &mut Vec::new(), true)?;
							}
							repeat_benchmark(repeat, &c, &mut results, false)?;
						}
					}
				}
				return Ok(results);
			}
		}
	};
}

// This creates a unit test for one benchmark of the main benchmark macro.
// It runs the benchmark using the `high` and `low` value for each component
// and ensure that everything completes successfully.
#[macro_export]
#[doc(hidden)]
macro_rules! impl_benchmark_test {
	(
		$runtime:ident
		$pallet:ident
		{ $( $instance:ident )? }
		$name:ident
	) => {
		$crate::paste::item! {
			fn [<test_benchmark_ $name>] () -> Result<(), &'static str>
			{
				let selected_benchmark = SelectedBenchmark::$name;
				let components = <
					SelectedBenchmark as $crate::BenchmarkingSetup<$runtime, _>
				>::components(&selected_benchmark);

				let execute_benchmark = |
					c: Vec<($crate::BenchmarkParameter, u32)>
				| -> Result<(), &'static str> {
					// Set up the benchmark, return execution + verification function.
					let closure_to_verify = <
						SelectedBenchmark as $crate::BenchmarkingSetup<$runtime, _>
					>::instance(&selected_benchmark, &c, true)?;

					// Set the block number to at least 1 so events are deposited.
					if $crate::Zero::is_zero(&frame_system::Module::<$runtime>::block_number()) {
						frame_system::Module::<$runtime>::set_block_number(1u32.into());
					}

					// Run execution + verification
					closure_to_verify()?;

					// Reset the state
					$crate::benchmarking::wipe_db();

					Ok(())
				};

				if components.is_empty() {
					execute_benchmark(Default::default())?;
				} else {
					for (_, (name, low, high)) in components.iter().enumerate() {
						// Test only the low and high value, assuming values in the middle won't break
						for component_value in vec![low, high] {
							// Select the max value for all the other components.
							let c: Vec<($crate::BenchmarkParameter, u32)> = components.iter()
								.enumerate()
								.map(|(_, (n, _, h))|
									if n == name {
										(*n, *component_value)
									} else {
										(*n, *h)
									}
								)
								.collect();

							execute_benchmark(c)?;
						}
					}
				}
				Ok(())
			}
		}
	};
}

/// This macro adds pallet benchmarks to a `Vec<BenchmarkBatch>` object.
///
/// First create an object that holds in the input parameters for the benchmark:
///
/// ```ignore
/// let params = (&config, &whitelist);
/// ```
///
/// The `whitelist` is a parameter you pass to control the DB read/write
/// tracking. We use a vector of [TrackedStorageKey], which is a simple struct
/// used to set if a key has been read or written to.
///
/// For values that should be skipped entirely, we can just pass `key.into()`.
/// For example:
///
/// ```
/// use frame_benchmarking::TrackedStorageKey;
/// use hex_literal;
/// let whitelist: Vec<TrackedStorageKey> = vec![
///     // Block Number
///     hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec().into(),
///     // Total Issuance
///     hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec().into(),
///     // Execution Phase
///     hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec().into(),
///     // Event Count
///     hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec().into(),
/// ];
/// ```
///
/// Then define a mutable local variable to hold your `BenchmarkBatch` object:
/// ```ignore
/// let mut batches = Vec::<BenchmarkBatch>::new();
/// ````
///
/// Then add the pallets you want to benchmark to this object, using their crate
/// name and generated module struct:
/// ```ignore
/// add_benchmark!(params, batches, pallet_balances, Balances);
/// add_benchmark!(params, batches, pallet_session, SessionBench::<Runtime>);
/// add_benchmark!(params, batches, frame_system, SystemBench::<Runtime>);
/// ...
/// ```
///
/// At the end of `dispatch_benchmark`, you should return this batches object.
#[macro_export]
macro_rules! add_benchmark {
	( $params:ident, $batches:ident, $name:ident, $( $location:tt )* ) => (
		let name_string = stringify!($name).as_bytes();
		let instance_string = stringify!( $( $location )* ).as_bytes();
		let (config, whitelist) = $params;
		let $crate::BenchmarkConfig {
			pallet,
			benchmark,
			lowest_range_values,
			highest_range_values,
			steps,
			repeat,
			verify,
			extra,
		} = config;
		if &pallet[..] == &name_string[..] || &pallet[..] == &b"*"[..] {
			if &pallet[..] == &b"*"[..] || &benchmark[..] == &b"*"[..] {
				for benchmark in $( $location )*::Benchmark::benchmarks(*extra).into_iter() {
					$batches.push($crate::BenchmarkBatch {
						instance: instance_string.to_vec(),
						results: $( $location )*::Benchmark::run_benchmark(
							benchmark,
							&lowest_range_values[..],
							&highest_range_values[..],
							&steps[..],
							*repeat,
							whitelist,
							*verify,
						)?,
						pallet: name_string.to_vec(),
						benchmark: benchmark.to_vec(),
					});
				}
			} else {
				$batches.push($crate::BenchmarkBatch {
					instance: instance_string.to_vec(),
					results: $( $location )*::Benchmark::run_benchmark(
						&benchmark[..],
						&lowest_range_values[..],
						&highest_range_values[..],
						&steps[..],
						*repeat,
						whitelist,
						*verify,
					)?,
					pallet: name_string.to_vec(),
					benchmark: benchmark.clone(),
				});
			}
		}
	)
}
