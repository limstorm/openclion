#include <iostream>

#include "greeter.h"

int main(int argc, char** argv) {
    const std::string name = argc > 1 ? argv[1] : "world";
    const acme::Greeter greeter(name);

    std::cout << greeter.greet() << '\n';
    return 0;
}
