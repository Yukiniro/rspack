import { BuiltinPluginName } from "@rspack/binding";

import { create } from "./base";

export const NoEmitOnErrorsPlugin = create(
	BuiltinPluginName.NoEmitOnErrorsPlugin,
	() => undefined
);
