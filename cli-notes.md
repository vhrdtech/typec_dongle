# Commands

## Reset Command
There should be a reset command to get everything back to its default state on the dongle.
A global (top level) one as well as for the relevant subcommands like gpio, usb, ...

* GPIO each pin should be configured as input
* USB

## USB Command
* Add support for PD commands


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




