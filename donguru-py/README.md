# donguru

Python bindings of the Donguru USB Type-C dongle: power, meter, USB data, header, UART and settings, as a blocking API with SI values as floats. Built with PyO3 and maturin over `donguru-core`.

```python
import donguru

with donguru.open("bench-1") as dg:
    dg.power.on(trip=1.5)
    for r in dg.meter.watch(interval=0.5, count=10):
        print(f"{r.voltage:.3f} V {r.current:.3f} A")
```

Documentation: <https://vhrdtech.github.io/typec_dongle/user/python/>
