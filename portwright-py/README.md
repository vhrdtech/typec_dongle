# portwright

Python bindings of the Portwright USB Type-C dongle: power, meter, USB data, header, UART and settings, as a blocking API with SI values as floats. Built with PyO3 and maturin over `portwright-core`.

```python
import portwright

with portwright.open("bench-1") as pw:
    pw.power.on(trip=1.5)
    for r in pw.meter.watch(interval=0.5, count=10):
        print(f"{r.voltage:.3f} V {r.current:.3f} A")
```

Documentation: <https://vhrdtech.github.io/typec_dongle/user/python/>
