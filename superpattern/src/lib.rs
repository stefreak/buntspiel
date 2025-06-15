use std;
use std::sync::LazyLock;

use ast_grep_core::language::TSLanguage;
use ast_grep_core::matcher::KindMatcher;
use ast_grep_core::traversal::Visitor;
use ast_grep_core::{Language, Matcher, Node, StrDoc};
use regex::Regex;

const JS: LazyLock<TSLanguage> = LazyLock::new(|| tree_sitter_javascript::language().into());

pub struct TransformationResult {
    pub state_vars: Vec<std::string::String>,
    pub globals: Vec<std::string::String>,
    pub transformed_pattern: std::string::String,
}
pub fn transform_pattern(str: &str) -> TransformationResult {
    let mut ast_grep = JS.ast_grep(str);

    let state_declaration_nodes: Vec<_> = ast_grep
        .root()
        .find_all("var $N")
        .filter(|m| {
            m.parent()
                .is_some_and(|p| p.node_id() == ast_grep.root().node_id())
        })
        .collect();

    let mut state_vars: Vec<_> = state_declaration_nodes
        .iter()
        .map(|m| {
            m.child(1)
                .unwrap()
                .field("name")
                .unwrap()
                .text()
                .to_string()
        })
        .collect();

    state_vars.dedup();

    let global_nodes: Vec<_> = ast_grep
        .root()
        .find_all("$N = $MATCH")
        .filter(|n| {
            if n.field("left").unwrap().kind() == "subscript_expression" {
                return false;
            }
            if n.kind() == "variable_declaration" {
                return false;
            }
            let varname = n.field("left").unwrap().text().to_string();
            let mut is_declared_locally = false;
            for ancestor in n.ancestors() {
                if ancestor.kind() == "function_declaration" {
                    break;
                }
                is_declared_locally = ancestor.prev_all().find_var_declaration(&varname);
                if is_declared_locally {
                    break;
                }
            }
            let is_declared_state = ast_grep.root().children().find_var_declaration(&varname);
            let is_parameter = n.ancestors().is_parameter_name(&varname);

            return !is_declared_locally && !is_declared_state && !is_parameter;
        })
        .collect();

    let mut globals: Vec<_> = global_nodes
        .iter()
        .map(|n| n.field("left").unwrap().text().to_string())
        .collect();

    globals.dedup();

    // Do we really need to replace one at a time, and clone every time?
    let mut iteration = 0;
    loop {
        iteration += 1;
        eprintln!(
            "DEBUG: Starting identifier transformation iteration {}",
            iteration
        );
        let clone = ast_grep.clone();
        if let None = Visitor::new(KindMatcher::from_id(
            JS.id_for_node_kind("identifier", true),
        ))
        .reentrant(false)
        .visit(clone.root())
        .find(|m| {
            let kind = m.kind();
            let parent_kind = m.parent().map(|p| p.kind().into_owned());
            let is_declaration = parent_kind.map(|k| k.contains("declarat")).unwrap_or(false);

            if kind == "identifier" && !is_declaration {
                let name = m.text().into_owned();

                // Debug output for ALL identifiers in iteration 2+
                if iteration >= 2 {
                    eprintln!(
                        "DEBUG: Found identifier '{}' at position {:?}",
                        name,
                        m.range()
                    );
                }

                // Debug output for the variables we're interested in
                if name == "state" || name == "globalVar" {
                    eprintln!(
                        "DEBUG: Processing identifier '{}' at position {:?}",
                        name,
                        m.range()
                    );
                }

                if m.ancestors().is_parameter_name(&name) {
                    if name == "state" || name == "globalVar" {
                        eprintln!("DEBUG: '{}' is a parameter, skipping", name);
                    }
                    return false;
                }

                let mut is_declared_locally = false;
                for ancestor in m.ancestors() {
                    if ancestor.kind() == "function_declaration" {
                        break;
                    }
                    is_declared_locally = ancestor.prev_all().find_var_declaration(&name);
                    if is_declared_locally {
                        break;
                    }
                }
                if is_declared_locally {
                    if name == "state" || name == "globalVar" {
                        eprintln!("DEBUG: '{}' is declared locally, skipping", name);
                    }
                    return false;
                }

                if globals.contains(&name) {
                    if name == "state" || name == "globalVar" {
                        eprintln!("DEBUG: '{}' found in globals, transforming", name);
                    }
                    ast_grep
                        .edit(
                            m.replace_by(
                                &format!(
                                    "__globals__[{}]",
                                    globals.iter().position(|g| g == &name).unwrap()
                                )
                                .as_str(),
                            ),
                        )
                        .unwrap();
                    return true;
                }

                if state_vars.contains(&name) {
                    if name == "state" || name == "globalVar" {
                        eprintln!("DEBUG: '{}' found in state_vars, transforming", name);
                    }
                    ast_grep
                        .edit(
                            m.replace_by(
                                &format!(
                                    "__state__[{}]",
                                    state_vars.iter().position(|g| g == &name).unwrap()
                                )
                                .as_str(),
                            ),
                        )
                        .unwrap();
                    return true;
                }

                if name == "state" || name == "globalVar" {
                    eprintln!("DEBUG: '{}' not found in globals or state_vars", name);
                }
            }

            false
        }) {
            break;
        };
    }
    loop {
        let clone = ast_grep.clone();
        if let None = Visitor::new("var $_NAME = $$$VALUE")
            .reentrant(false)
            .visit(clone.root())
            .find(|m| {
                let Some(parent) = m.parent() else {
                    return false;
                };
                let parent_node_id = parent.node_id();
                let root_node_id = clone.root().node_id();
                let _sexp = m.to_sexp();
                let _sexp_parent = parent.to_sexp();

                // Only process root-level variable declarations
                if parent_node_id == root_node_id {
                    let identifier = m.child(1).unwrap().field("name").unwrap().text();

                    // Only transform if this is actually a state variable
                    if let Some(pos) = state_vars.iter().position(|g| g == &identifier) {
                        ast_grep
                            .edit(m.replace_by(&format!("__state__[{}] = $$$VALUE", pos).as_str()))
                            .unwrap();
                        return true;
                    }
                    // If it's a root-level var but not in state_vars, just skip it
                    // (this shouldn't happen with proper logic, but let's be safe)
                }

                false
            })
        {
            break;
        };

        loop {
            let clone = ast_grep.clone();
            if let None = Visitor::new("$FUNC($$$ARGS)")
                .reentrant(false)
                .visit(clone.root())
                .find(|m| {
                    let identifier = m.field("function").unwrap();
                    let args = m.field("arguments").unwrap();

                    // stop infinite loop
                    if args.text().contains("__state__") {
                        return false;
                    }

                    let _foo = m.to_sexp();

                    if m.ancestors()
                        .find_func_declaration(&identifier.text().into_owned())
                    {
                        ast_grep
                            .edit(m.replace_by(&"$FUNC(__state__, __globals__, $$$ARGS)"))
                            .unwrap();
                        return true;
                    }

                    return false;
                })
            {
                break;
            };
        }
    }

    // TODO: Why doesn't it match the `render` function in the test cases below?
    // we are working around an issue in ast-grep using the regular expression.
    // In the playground the replacement rule is working as expected: https://ast-grep.github.io/playground.html#eyJtb2RlIjoiUGF0Y2giLCJsYW5nIjoiamF2YXNjcmlwdCIsInF1ZXJ5IjoiZnVuY3Rpb24gJEEoJCQkUEFSQU1TKSB7ICQkJEJPRFkgfSIsInJld3JpdGUiOiJmdW5jdGlvbiAkQSgkJCRQQVJBTVMpIHsgJCQkQk9EWSB9IiwiY29uZmlnIjoiIiwic291cmNlIjoiICAgICAgICB2YXIgc3RhdGUxO1xuICAgICAgICB2YXIgc3RhdGUyID0gbnVsbFxuICAgICAgICB2YXIgc3RhdGVfZnJvbV9pbm5lcjE7XG4gICAgICAgIHZhciBzdGF0ZV9mcm9tX2lubmVyMjtcblxuICAgICAgICBmdW5jdGlvbiByZW5kZXIocGFyYW0xLCBwYXJhbTIpIHtcbiAgICAgICAgICAgIGdsb2JhbCA9IG51bGxcbiAgICAgICAgICAgIHBhcmFtMSA9IG51bGxcbiAgICAgICAgICAgIHBhcmFtMlsndGVzdCddID0gbnVsbFxuICAgICAgICAgICAgc3RhdGUxID0gbnVsbFxuICAgICAgICAgICAgc3RhdGUyID0gbnVsbFxuICAgICAgICAgICAgdmFyIGRlY2xhcmVkTG9jYWwxID0gbnVsbFxuICAgICAgICAgICAgdmFyIGRlY2xhcmVkTG9jYWwyO1xuICAgICAgICAgICAgZGVjbGFyZWRMb2NhbDIgPSBudWxsXG4gICAgICAgICAgICB2YXIgYWN0dWFsbHlHbG9iYWwxID0gbnVsbFxuICAgICAgICAgICAgdmFyIGFjdHVhbGx5R2xvYmFsMjtcbiAgICAgICAgICAgIGZ1bmN0aW9uIGlubmVyX2Z1bigpIHtcbiAgICAgICAgICAgICAgICAvLyBUaGlzIGlzIGFjdHVhbGx5IGEgZ2xvYmFsIGFzc2lnbm1lbnQsIGJlY2F1c2UgZnVuY3Rpb25zIGluIHRoZSBQaXhlbGJsYXplIHBhdHRlcm4gbGFuZ3VhZ2UgZG8gbm90IGNhcHR1cmUgdmFyaWFibGUgc2NvcGVcbiAgICAgICAgICAgICAgICBhY3R1YWxseUdsb2JhbDEgPSBudWxsXG4gICAgICAgICAgICAgICAgYWN0dWFsbHlHbG9iYWwyID0gbnVsbFxuICAgICAgICAgICAgICAgIHN0YXRlX2Zyb21faW5uZXIxID0gbnVsbFxuICAgICAgICAgICAgICAgIHN0YXRlX2Zyb21faW5uZXIyID0gbnVsbFxuICAgICAgICAgICAgfVxuICAgICAgICAgICAgZnVuY3Rpb24gaW5uZXJfZnVuMihuYW1lKSB7XG4gICAgICAgICAgICAgICAgY29uc29sZS5sb2cobmFtZSlcbiAgICAgICAgICAgIH1cbiAgICAgICAgICAgIGlubmVyX2Z1bigpXG4gICAgICAgICAgICAvLyBzaG91bGQgbGVhdmUgdGhpcyBhbG9uZSwgYXMgaXQgaGFzbid0IGJlZW4gZGVjbGFyZWQgaW4gdGhpcyBwYXR0ZXJuXG4gICAgICAgICAgICBoc3YoMSwyLDMpXG4gICAgICAgIH1cbiAgICAgICAgICAgICAgICBmdW5jdGlvbiBpbm5lcl9mdW40KG5hbWUpIHtcbiAgICAgICAgICAgIGNvbnNvbGUubG9nKG5hbWUpXG4gICAgICAgIH1cbiJ9
    // loop {
    //     let clone = ast_grep.clone();
    //     if let None = Visitor::new("function $FUNC($$$ARGS){$$$BODY}")
    //         .reentrant(false)
    //         .visit(clone.root())
    //         .find(|m| {
    //             let _name = m.get_env().get_match("FUNC").unwrap().text();
    //             let already_replaced = m
    //                 .get_env()
    //                 .get_multiple_matches("ARGS")
    //                 .iter()
    //                 .any(|n| n.text().contains("__state__"));

    //             // stop infinite loop
    //             if already_replaced {
    //                 return false;
    //             }

    //             ast_grep
    //                 .edit(m.replace_by("function $FUNC(__state__, __globals__, $$$ARGS){$$$BODY}"))
    //                 .unwrap();

    //             return true;
    //         })
    //     {
    //         break;
    //     };
    // }

    let regex = Regex::new(r"function\W+([a-zA-Z0-9-_]+)\W*\((.*)\)\W*\{").unwrap();
    let transformed_pattern = regex
        .replace_all(
            &ast_grep.generate(),
            "function $1(__state__, __globals__, $2) {",
        )
        .to_string();

    TransformationResult {
        state_vars,
        globals,
        transformed_pattern,
    }
}

trait AstHelpers<'a, Lang>
where
    Lang: ast_grep_core::Language + 'a,
{
    fn find_func_declaration(&mut self, varname: &String) -> bool;
    fn is_parameter_name(&mut self, name: &str) -> bool;
    fn find_var_declaration(&mut self, varname: &String) -> bool;
}

impl<'a, Lang, T: Iterator<Item = Node<'a, StrDoc<Lang>>>> AstHelpers<'a, Lang> for T
where
    Lang: ast_grep_core::Language + 'a,
{
    fn find_var_declaration(&mut self, varname: &String) -> bool {
        self.filter_map(|n| "var $N".match_node(n.into()))
            .find(|n| {
                if n.kind_id() != JS.id_for_node_kind("variable_declaration", true) {
                    return false;
                }
                let child = n.child(1).unwrap();
                let declared_varname = child.field("name").unwrap().text();
                let did_match = declared_varname == *varname;
                did_match
            })
            .is_some()
    }

    fn find_func_declaration(&mut self, funcname: &String) -> bool {
        self.filter_map(|n| "function $FUNC($$$ARGS) { $$$ }".match_node(n.into()))
            .find(|n| {
                if n.kind_id() != JS.id_for_node_kind("function_declaration", true) {
                    return false;
                }
                let declared_funcname = n.field("name").unwrap().text();
                let did_match = declared_funcname == *funcname;
                did_match
            })
            .is_some()
    }

    fn is_parameter_name(&mut self, name: &str) -> bool {
        let parameter_names = self
            .find_map(|n| "function $FUNC($$$ARGS) { $$$ }".match_node(n))
            .map(|n| {
                let params = n.field("parameters").unwrap();

                let mut param_names = vec![];

                for param in params.children().filter_map(|n| {
                    let kind = n.kind();
                    if kind == "identifier" {
                        return Some(n.text().to_string());
                    }
                    None
                }) {
                    param_names.push(param.clone());
                }
                param_names
            });

        let is_parameter = parameter_names
            .map(|params| params.contains(&&name.to_string()))
            .unwrap_or(false);

        return is_parameter;
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;

    // Task 1: Variable Classification Tests
    // These tests verify that the AST transformation correctly identifies different types of variables
    // for isolation in superpatterns where multiple patterns are combined.

    /// Test that variables declared with 'var' at the root level are correctly identified as state variables.
    /// State variables are pattern-specific and need to be isolated so that multiple patterns can
    /// coexist without variable name conflicts. Each pattern gets its own state array slice.
    #[test]
    fn test_state_variable_detection() {
        let code = "
            var stateVar1;
            var stateVar2 = 42;
            var stateVar3 = null;
        ";
        let res = transform_pattern(code);
        assert_eq!(res.state_vars, vec!["stateVar1", "stateVar2", "stateVar3"]);
    }

    /// Test that variables assigned without declaration are correctly identified as global variables.
    /// Global variables are also pattern-specific and need to be isolated so that multiple patterns
    /// can coexist without variable name conflicts. Each pattern gets its own globals array slice.
    #[test]
    fn test_global_variable_detection() {
        let code = "
            globalVar1 = 42;
            globalVar2 = array(10);
            globalVar3 = null;
        ";
        let res = transform_pattern(code);
        assert_eq!(res.globals, vec!["globalVar1", "globalVar2", "globalVar3"]);
    }

    /// Test that variables declared inside functions are correctly preserved as local variables.
    /// Local variables should NOT be transformed since they are already isolated by function scope
    /// and don't cause conflicts between patterns.
    #[test]
    fn test_local_variable_preservation() {
        let code = "
            function render() {
                var localVar1 = 42;
                var localVar2;
                localVar1 = 100;
                localVar2 = null;
            }
        ";
        let res = transform_pattern(code);
        // Local variables should not appear in state_vars or globals
        assert_eq!(res.state_vars, Vec::<String>::new());
        assert_eq!(res.globals, Vec::<String>::new());
        // The transformed code should preserve local variable names exactly
        assert_eq!(
            res.transformed_pattern,
            "
            function render(__state__, __globals__, ) {
                var localVar1 = 42;
                var localVar2;
                localVar1 = 100;
                localVar2 = null;
            }
        "
        );
    }

    /// Test that function parameters are correctly preserved and not transformed.
    /// Parameters should NOT be transformed since they are already isolated by function scope
    /// and don't cause conflicts between patterns.
    #[test]
    fn test_parameter_variable_detection() {
        let code = "
            function render(index, x, y) {
                index = index + 1;
                x = x * 2;
                y = y / 2;
            }
        ";
        let res = transform_pattern(code);
        // Parameters should not appear in state_vars or globals
        assert_eq!(res.state_vars, Vec::<String>::new());
        assert_eq!(res.globals, Vec::<String>::new());
        // The transformed code should preserve parameter names exactly
        assert_eq!(
            res.transformed_pattern,
            "
            function render(__state__, __globals__, index, x, y) {
                index = index + 1;
                x = x * 2;
                y = y / 2;
            }
        "
        );
    }

    // Task 2: Variable Scoping Edge Cases
    // These tests verify that the AST transformation correctly handles complex scoping scenarios
    // that can occur in real Pixelblaze patterns.

    /// Test variable shadowing where local variables shadow state variables.
    /// The local variable should take precedence and NOT be transformed, while the state
    /// variable should still be detected and available for transformation elsewhere.
    #[test]
    fn test_variable_shadowing() {
        let code = "
            var state = 42;
            globalVar = 100;
            function render(state) {
                var globalVar = 200;
                state = 300;        // This should refer to parameter, not state var
                globalVar = 400;    // This should refer to local var, not global
            }
            function other() {
                state = 500;        // This should transform to __state__[0]
                globalVar = 600;    // This should transform to __globals__[0]
            }
        ";
        let res = transform_pattern(code);
        assert_eq!(res.state_vars, vec!["state"]);
        assert_eq!(res.globals, vec!["globalVar"]);
        assert_eq!(
            res.transformed_pattern,
            "
            __state__[0] = 42
            __globals__[0] = 100;
            function render(__state__, __globals__, state) {
                var globalVar = 200;
                state = 300;        // This should refer to parameter, not state var
                globalVar = 400;    // This should refer to local var, not global
            }
            function other(__state__, __globals__, ) {
                __state__[0] = 500;        // This should transform to __state__[0]
                __globals__[0] = 600;    // This should transform to __globals__[0]
            }
        "
        );
    }

    const LOCAL_GLOBAL_PARAM1_PARAM2: &str = "
        var state1;
        var state2 = null
        var state_from_inner1;
        var state_from_inner2;
        var state_funs = array(2);
        state_funs[0] = (index) => state_funs[index]()
        global_funs = array(2)
        global_funs[0] = (index) => global_funs[index]()
        param_global = null
        var param_state = null

        function render(param1, param2) {
            global = null
            param1 = null
            param2['test'] = null
            state1 = null
            state2 = null
            var declaredLocal1 = null
            var declaredLocal2;
            declaredLocal2 = null
            var actuallyGlobal1 = null
            var actuallyGlobal2;
            function inner_fun() {
                // This is actually a global assignment, because functions in the Pixelblaze pattern language do not capture variable scope
                actuallyGlobal1 = null
                actuallyGlobal2 = null
                state_from_inner1 = null
                state_from_inner2 = null
            }
            inner_fun()
            state_funs[1] = inner_fun
            state_funs[1]()
            global_funs[1] = inner_fun
            global_funs[1]()

            // should leave this alone, as it hasn't been declared in this pattern
            hsv(1,2,3)
        }
        function render2(param_global, param_state) {
            // should leave this alone, as it hasn't been declared in this pattern
            hsv(param_state,param_global,3)

            param_state = null
            param_global = null
        }";

    #[test]
    fn can_identify_root_locals() {
        let res = transform_pattern(LOCAL_GLOBAL_PARAM1_PARAM2);
        assert_eq!(
            res.state_vars,
            vec![
                "state1",
                "state2",
                "state_from_inner1",
                "state_from_inner2",
                "state_funs",
                "param_state"
            ]
        );
    }
    #[test]
    fn can_identify_globals() {
        let res = transform_pattern(LOCAL_GLOBAL_PARAM1_PARAM2);
        assert_eq!(
            res.globals,
            vec![
                "global_funs",
                "param_global",
                "global",
                "actuallyGlobal1",
                "actuallyGlobal2"
            ]
        );
    }

    #[test]
    fn transforms_code() {
        let res = transform_pattern(LOCAL_GLOBAL_PARAM1_PARAM2);
        assert_eq!(res.transformed_pattern,"
        var state1;
        __state__[1] = null
        var state_from_inner1;
        var state_from_inner2;
        __state__[4] = array(2)
        __state__[4][0] = (__state__, __globals__, index) => __state__[4][index]()
        __globals__[0] = array(2)
        __globals__[0][0] = (__state__, __globals__, index) => __globals__[0][index]()
        __globals__[1] = null
        var param_state = null

        function render(__state__, __globals__, param1, param2) {
            __globals__[1] = null
            param1 = null
            param2['test'] = null
            __state__[0] = null
            __state__[1] = null
            var declaredLocal1 = null
            var declaredLocal2;
            declaredLocal2 = null
            var actuallyGlobal1 = null
            var actuallyGlobal2;
            function inner_fun(__state__, __globals__, ) {
                // This is actually a global assignment, because functions in the Pixelblaze pattern language do not capture variable scope
                __globals__[2] = null
                __globals__[3] = null
                __state__[2] = null
                __state__[3] = null
            }
            inner_fun(__state__, __globals__, )
            // should leave this alone, as it hasn't been declared in this pattern
            hsv(1,2,3)
        }
        function render2(__state__, __globals__, ) {
            // should leave this alone, as it hasn't been declared in this pattern
            hsv(1,2,3)
        }");
    }
}
