#include <cassert>
#include <string>

#include "greeter.h"

int main() {
    const acme::Greeter greeter("Ada");
    assert(greeter.greet() == "Hello, Ada!");
    return 0;
}
