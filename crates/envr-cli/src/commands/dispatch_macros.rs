macro_rules! dispatch_match {
    ($command:expr, _ => $fallback:expr; $( $pat:pat => $handler:expr ),+ $(,)?) => {{
        match $command {
            $( $pat => $handler, )+
            _ => $fallback,
        }
    }};

    ($command:expr; $( $pat:pat => $handler:expr ),+ $(,)?) => {{
        match $command {
            $( $pat => $handler, )+
        }
    }};
}

pub(crate) use dispatch_match;
