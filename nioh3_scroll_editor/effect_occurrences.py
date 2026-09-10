"""Match display-slot requirements without reusing an effect occurrence."""

def matches_occurrences(effects, requirements):
    choices = []
    for requirement in requirements:
        eligible = []
        for slot, effect in enumerate(effects):
            if requirement['scope'] == 'primary' and slot != 0:
                continue
            if requirement['scope'] == 'secondary' and slot == 0:
                continue
            if any(effect.effect_id == choice['effect_id'] and
                   (choice['minimum_roll_percent'] == 0 or effect.roll_percent is not None and
                    effect.roll_percent >= choice['minimum_roll_percent'])
                   for choice in requirement['alternatives']):
                eligible.append(slot)
        if not eligible:
            return False
        choices.append(eligible)
    choices.sort(key=len)

    def assign(index, used):
        if index == len(choices):
            return True
        return any(assign(index + 1, used | (1 << slot))
                   for slot in choices[index] if not used & (1 << slot))

    return assign(0, 0)
