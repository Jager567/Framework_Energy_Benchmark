const { exec } = require("child_process")

const express = require("express")
const app = express()
const port = 3000

const load_generator_path = "../load-generator/target/release/load-generator"

app.get("/", (req, res) => {
	/*
	-c, --connections <CONNECTIONS>                  [default: 500]
  -r, --requests-per-second <REQUESTS_PER_SECOND>  [default: 20000]
  -t, --test-duration <TEST_DURATION>              [default: 1000000000]
      --startup-delay <STARTUP_DELAY>              [default: 100000000]
 	*/
	if (!req.query.target) {
		return res.status(400).send("Need a target!")
	}

	let cmd = load_generator_path
	if (req.query.connections) {
		cmd += ` -c ${req.query.connections}`
	}
	if (req.query.rps) {
		cmd += ` -r ${req.query.rps}`
	}
	if (req.query.duration) {
		cmd += ` -t ${req.query.duration}`
	}
	if (req.query.delay) {
		cmd += ` --startup-delay ${req.query.delay}`
	}
	cmd += ` ${req.query.target}`

	exec(cmd, (error, stdout, stderr) => {
		if (error) {
			res.write(`There was an error executing the command:\n${error.message}`)
		}
		if (stderr) {
			res.write(`stderr:\n${stderr}`)
		}
		if (stdout) {
			res.write(`stdout:\n${stdout}`)
		}
		res.end()
	})
	res.write(`Hello, World! You supplied the following parameters:\n${JSON.stringify(req.query)}\n`)
})

app.listen(port, () => {
	console.log(`Example app listening on port ${port}`)
})
