frameworks="express laravel actix django aspcore"
order=$(yes $frameworks | head -n $1 | sed -r 's/ /\n/g' | shuf | sed -r 's/\n/ /g')

#FIB HERE

# 1: number of repetitions
# 2: startup wait
# 3: warmup duration
# 4: test duration
# 5: load-generator server url

printf "" >| output.csv

echo "Stopping frameworks"
docker stop $frameworks > /dev/null

my_ip="$(ip -4 a show enp3s0  | grep -oP '(?<=inet\s)\d+(\.\d+){3}')"
echo "My ip is ${my_ip}"

for framework in $order ; do
    printf "Starting $framework\n"
    docker start $framework > /dev/null
    printf "Waiting for $framework to start\n"
    sleep $2

    printf "Warmup $framework\n"
    curl --no-progress-meter "http://${5}:3000/?connections=16&rps=500&duration=${3}000000&delay=0&target=http%3A%2F%2F${my_ip}%3A8080%2Fjson"

    {
        printf $framework >> output.csv
        printf "," >> output.csv
        echo "starting energy measurement"
        energyconsumed=$(./rapl-powertool/rapl-powertool INTEL -d $(($4+10000)))
        echo "finished energy measurement, consumed ${energyconsumed}j"
        printf $energyconsumed >> output.csv
        printf "\n" >> output.csv
    } & {
        echo "starting loadtest"
        curl --no-progress-meter "http://${5}:3000/?connections=100&rps=3000&duration=${4}000000&delay=0&target=http%3A%2F%2F${my_ip}%3A8080%2Fjson"
        echo "finished loadtest"
    } & wait
    
    docker kill $framework > /dev/null
    sleep 1
done
