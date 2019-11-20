macro_rules! err {
    ($( $tok:ident ).+ : $fstr:literal$(,)? $( $arg:expr ),*) => {
        err!([error] $($tok).+ : $fstr, $($arg),*)
    };
    ([ $level:ident ] $( $tok:ident ).+ : $fstr:literal$(,)? $( $arg:expr ),*) => {
        $($tok).+.span().unwrap().$level(format!($fstr, $($arg),*)).emit();
    };
}
