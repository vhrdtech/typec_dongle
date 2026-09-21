# Commands

## Reset Command
There should be a reset command to get everything back to its default state on the dongle.
A global (top level) one as well as for the relevant subcommands like gpio, usb, ...

* GPIO each pin should be configured as input
* USB

## USB Command
* Add support for PD commands

## GPIO Command

### Write
* Consider adding supporting writing multiple pins in one command
* Consider support for custom syntax like <PIN>:<LEVEL" e.g. "p1:high" 
  using this "p1:low p2:high" it would be easiert to specify multiple pins at once

### Configure
* Similar to write consider configuring multiple gpios at once
* Also consider using custom config strings e.g. p1:out:low ...


## Firmware

* firmware
    - version
    - update


# Extension
* power
* i2c, uart  (streaming manner)
* udev



    /// Power switch and power meter
    #[command(subcommand)]
    Power,
     watch

    /// I2C bus bridge
    #[command(subcommand)]
    I2c,

    /// UART (USART) bridge
    #[command(subcommand)]
    Uart,

    /// Firmware version and update
    #[command(subcommand)]
    Firmware,



## Add global flags/options
-c, --config


## Interactive mode like ka3005p



## Additonal commands

* Theme
    show
    set
    create ...




