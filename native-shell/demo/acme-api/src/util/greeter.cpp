#include "greeter.h"

#include <string>

namespace acme {

Greeter::Greeter(std::string name) : name_(std::move(name)) {}

std::string Greeter::greet() const {
    return "Hello, " + name_ + "!";
}

}  // namespace acme
