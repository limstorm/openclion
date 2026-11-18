#ifndef ACME_GREETER_H_
#define ACME_GREETER_H_

#include <string>

namespace acme {

class Greeter {
public:
    explicit Greeter(std::string name);

    std::string greet() const;

private:
    std::string name_;
};

}  // namespace acme

#endif  // ACME_GREETER_H_
