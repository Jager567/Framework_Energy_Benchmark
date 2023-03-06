# Web Application Framework Power Consumption Benchmark

This project provides simple tools to measure and compare power consumption of
various web application frameworks. It utilises the [TechEmpower Framework
Benchmarks](https://www.techempower.com/benchmarks/) benchmark suite for
representative implementations of a reference endpoint in various frameworks
and languages. Right now, we are only able to measure the `/json` endpoint,
which demonstrates a frameworks ability to serialize a JSON object.

## How to clone

This repository makes use of git submodules. Either add `--recurse-submodules`
to your clone command, or when you've cloned the repository, run `git submodule
update --init`.

## How to run

When you measure power consumption, you measure power consumption of the entire
system. To ensure one collects minimal noise, you need to minimise all activity
that is not related to the web framework tested. This is why you need two
machines, one to generate the load for the frameworks, the other to run the
webserver tested. You need to ensure both machines can reach eachother over the
internet, but preferably over local ethernet. The setup process for each
machine is outlined below.

### Load Generator machine

This machine will run the `load-generator` and the `load-generator-launcher`. To
build the `load-generator`, run `cargo build --release` in the `load-generator`
directory. Next, run `node index.js` in the `load-generator-launcher`
directory. The load generator is now ready to accept requests.

### Framework Machine

First build the `rapl-powertool`, instructions can be found in
`rapl-powertool/README.md`. Next, build all required docker images and create
the containers by running `custom_docker_setup.sh` without arguments. Lastly,
start the benchmark by running `custom_docker_runner.sh`. Pass five arguments:

1. number of repetitions
1. startup wait (in seconds)
1. warmup duration  (in 1/1000 seconds)
1. test duration (in 1/1000 seconds)
1. load-generator server ip address

You can find the arguments we used for the paper in `command.sh`. You might need
to edit the `my_ip` declaration in `custom_docker_runner.sh` to use the correct
interface. To edit which frameworks are tested, change the first line in
`custom_docker_runner.sh` and `custom_docker_setup.sh`.
