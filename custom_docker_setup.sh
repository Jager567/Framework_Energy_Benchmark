frameworks="express laravel actix django aspcore"

for framework in $frameworks ; do
    for buildfile in FrameworkBenchmarks/frameworks/*/*/$framework.dockerfile ; do
        folder=$(dirname $buildfile)
        echo Building $framework
        docker build -f $buildfile -t $framework -q $folder
    done

    docker create -p 8080:8080 --init --name $framework $framework
done
