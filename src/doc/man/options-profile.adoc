*--profile* _NAME_::
    Changes convert:lowercase[{actionverb}] behavior. Currently only `test` is
    supported, which will convert:lowercase[{actionverb}] with the
    `#[cfg(test)]` attribute enabled. This is useful to have it
    convert:lowercase[{actionverb}] unit tests which are usually excluded via
    the `cfg` attribute. This does not change the actual profile used.
