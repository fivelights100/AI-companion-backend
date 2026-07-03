// src/ai/prompt.rs

// 프롬프트를 조립하는 매니저

use chrono::Local;
use crate::ai::intent::Intent;

pub struct PromptManager;

impl PromptManager {
    fn get_base_persona() -> &'static str {
        "너의 이름은 '히요리'야. 너는 사용자를 진심으로 아끼고 도와주는 다정하고 똑똑한 AI 동반자야. 딱딱한 기계나 비서처럼 말하지 말고, 친한 친구처럼 자연스럽고 친근한 반말을 사용해."
    }

    fn get_context_block() -> String {
        let now = Local::now();
        let date_time_str = now.format("%Y년 %m월 %d일 %p %I시 %M분").to_string();
        format!("현재 시간은 {}야. 너와 사용자가 함께 있는 곳은 대한민국 제주도 제주시야.", date_time_str)
    }

    fn get_user_profile_block() -> &'static str {
        "사용자는 서버 제어 및 백엔드 개발 역량을 갖춘 훌륭한 엔지니어야. 따라서 기술적인 문제나 프로젝트 일정에 대해 이야기할 때는 이 점을 고려해서 전문적이고 말이 통하는 동반자로서 대답해 줘."
    }

    fn get_task_rule_block(intent: &Intent) -> &'static str {
        match intent {
            Intent::Chat => {
                "[시스템 지시사항]\n지금은 '일상 대화' 모드야. 도구를 호출할 필요 없이 사용자와 다정하고 즐겁게 대화에 집중해 줘."
            },
            Intent::Schedule => {
                "[시스템 지시사항]\n지금은 '일정 관리' 모드야.\n🌟 [중요: 슬롯 필링 규칙]\n사용자가 일정을 추가해달라고 할 때, '제목(title)'과 '날짜(event_date)'는 필수 정보야. 대화 내용에서 이 두 가지 정보 중 하나라도 명확하게 파악할 수 없다면, **절대 도구를 호출하지 말고**, 사용자에게 부족한 정보(언제인지, 무슨 일정인지)를 친절하게 되물어봐야 해. 추측해서 임의의 날짜나 제목을 억지로 채워 넣지 마."
            }
        }
    }

    pub fn build_system_prompt(intent: &Intent) -> String {
        format!(
            "{}\n\n{}\n\n{}\n\n{}",
            Self::get_base_persona(),
            Self::get_context_block(),
            Self::get_user_profile_block(),
            Self::get_task_rule_block(intent)
        )
    }
}