# Aleoscan
Backend implemented in rust. Currently we support two functions: Peer node map, Navigation.

# Config
Set port and host in .env file. For example
```
HOST=0.0.0.0
PORT=9732
```

# Compile and run
- compile
```
cargo build --release
```
If openssl error occurs please install openssl dependency. 
```
# on ubuntu
sudo apt install libssl-dev
```
- run
Binary program can be found as /target/release/aleoscan-backend
```
./aleoscan-backend
```
Will run backend on port given in .env file.

![image](https://user-images.githubusercontent.com/98807352/200171601-af5eebdd-7787-4959-926b-bb904cadab47.png)

![image](https://user-images.githubusercontent.com/98807352/200171574-1e241d83-b0de-43b0-8d0f-eefb898c1678.png)
