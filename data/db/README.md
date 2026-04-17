# Bundled lensfun database

These XML files come from the upstream [LensFun](https://github.com/lensfun/lensfun) project under the **CC-BY-SA 3.0** license. They are bundled with `lensfun-rs` for convenience and to support the integration tests.

`timestamp.txt` records the upstream snapshot epoch.

## Attribution

The lens calibration data is the work of the LensFun community. Please credit them when redistributing or modifying these files (CC-BY-SA 3.0).

## Updating

```bash
# From the repo root:
rsync -a --delete related-repos/lensfun/data/db/ data/db/
```

(The `related-repos/` clone is gitignored; see `CONTRIBUTING.md`.)
