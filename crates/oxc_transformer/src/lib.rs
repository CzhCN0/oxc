#![allow(clippy::wildcard_imports, clippy::option_map_unit_fn)]

//! Transformer / Transpiler
//!
//! References:
//! * <https://www.typescriptlang.org/tsconfig#target>
//! * <https://babel.dev/docs/presets>
//! * <https://github.com/microsoft/TypeScript/blob/main/src/compiler/transformer.ts>

mod es2015;
mod es2016;
mod es2019;
mod es2021;
mod es2022;
mod options;
mod react_jsx;
mod regexp;
#[cfg(test)]
mod tester;
mod typescript;

use std::{cell::RefCell, rc::Rc};

use oxc_allocator::{Allocator, Vec};
use oxc_ast::{ast::*, AstBuilder, VisitMut};
use oxc_semantic::SymbolTable;
use oxc_span::SourceType;

use crate::{
    es2015::ShorthandProperties, es2016::ExponentiationOperator, es2019::OptionalCatchBinding,
    es2021::LogicalAssignmentOperators, react_jsx::ReactJsx, regexp::RegexpFlags,
    typescript::TypeScript,
};

pub use crate::options::{
    TransformOptions, TransformReactOptions, TransformReactRuntime, TransformTarget,
};

#[derive(Default)]
pub struct Transformer<'a> {
    #[allow(unused)]
    typescript: Option<TypeScript<'a>>,
    #[allow(unused)]
    react_jsx: Option<ReactJsx<'a>>,
    regexp_flags: Option<RegexpFlags<'a>>,
    // es2022
    es2022_class_static_block: Option<es2022::ClassStaticBlock<'a>>,
    // es2021
    es2021_logical_assignment_operators: Option<LogicalAssignmentOperators<'a>>,
    // es2019
    es2019_optional_catch_binding: Option<OptionalCatchBinding<'a>>,
    // es2016
    es2016_exponentiation_operator: Option<ExponentiationOperator<'a>>,
    // es2015
    es2015_shorthand_properties: Option<ShorthandProperties<'a>>,
}

impl<'a> Transformer<'a> {
    #[rustfmt::skip]
    pub fn new(
        allocator: &'a Allocator,
        source_type: SourceType,
        symbols: &Rc<RefCell<SymbolTable>>,
        options: TransformOptions,
    ) -> Self {
        let ast = Rc::new(AstBuilder::new(allocator));
        Self {
            typescript: source_type.is_typescript().then(|| TypeScript::new(Rc::clone(&ast))),
            react_jsx: options.react.map(|options| ReactJsx::new(Rc::clone(&ast), options)),
            regexp_flags: RegexpFlags::new(Rc::clone(&ast), options.target),
            es2022_class_static_block: (options.target < TransformTarget::ES2022).then(|| es2022::ClassStaticBlock::new(Rc::clone(&ast))),
            es2021_logical_assignment_operators: (options.target < TransformTarget::ES2021).then(|| LogicalAssignmentOperators::new(Rc::clone(&ast))),
            es2019_optional_catch_binding: (options.target < TransformTarget::ES2019).then(|| OptionalCatchBinding::new(Rc::clone(&ast))),
            es2016_exponentiation_operator: (options.target < TransformTarget::ES2016).then(|| ExponentiationOperator::new(Rc::clone(&ast), Rc::clone(symbols))),
            es2015_shorthand_properties: (options.target < TransformTarget::ES2015).then(|| ShorthandProperties::new(Rc::clone(&ast))),
        }
    }

    pub fn build(mut self, program: &mut Program<'a>) {
        self.visit_program(program);
    }
}

impl<'a> VisitMut<'a> for Transformer<'a> {
    fn visit_statements(&mut self, stmts: &mut Vec<'a, Statement<'a>>) {
        for stmt in stmts.iter_mut() {
            self.visit_statement(stmt);
        }
        self.es2016_exponentiation_operator.as_mut().map(|t| t.leave_statements(stmts));
    }

    fn visit_expression(&mut self, expr: &mut Expression<'a>) {
        // self.typescript.as_mut().map(|t| t.transform_expression(expr));
        // self.react_jsx.as_mut().map(|t| t.transform_expression(expr));
        self.regexp_flags.as_mut().map(|t| t.transform_expression(expr));

        self.es2021_logical_assignment_operators.as_mut().map(|t| t.transform_expression(expr));
        self.es2016_exponentiation_operator.as_mut().map(|t| t.transform_expression(expr));

        self.visit_expression_match(expr);
    }

    fn visit_catch_clause(&mut self, clause: &mut CatchClause<'a>) {
        self.es2019_optional_catch_binding.as_mut().map(|t| t.transform_catch_clause(clause));

        if let Some(param) = &mut clause.param {
            self.visit_binding_pattern(param);
        }
        self.visit_statements(&mut clause.body.body);
    }

    fn visit_object_property(&mut self, prop: &mut ObjectProperty<'a>) {
        self.es2015_shorthand_properties.as_mut().map(|t| t.transform_object_property(prop));

        self.visit_property_key(&mut prop.key);
        self.visit_expression(&mut prop.value);
        if let Some(init) = &mut prop.init {
            self.visit_expression(init);
        }
    }

    fn visit_class_body(&mut self, class_body: &mut ClassBody<'a>) {
        self.es2022_class_static_block.as_mut().map(|t| t.transform_class_body(class_body));

        class_body.body.iter_mut().for_each(|class_element| {
            self.visit_class_element(class_element);
        });
    }
}
